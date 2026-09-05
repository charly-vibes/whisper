//! Core deterministic logic: facts derivation, scope routing, layout ops.
//!
//! Everything here is a pure function of (git facts, config). The same
//! checkout must always resolve to the same paths on every machine.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::config::Resolved;
use crate::{Result, WhisperError};

/// Deterministic facts about the current checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facts {
    /// Canonical repo key, e.g. `github.com/u/r`, `cv/org/repo`, `local/name`.
    pub repo_key: String,
    /// Branch slug: `feature/x` → `feature--x`.
    pub branch_slug: String,
    /// Worktree slot: basename of the `.git` common dir, or of cwd.
    pub worktree_slot: String,
}

// ---------------------------------------------------------------------------
// Git fact derivation
// ---------------------------------------------------------------------------

/// Run `git` in `dir`, returning trimmed stdout or `None` on any failure.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Canonical repo key from a remote URL. Pure function.
///
/// - `git@host:org/repo.git` → `host/org/repo`
/// - `https://host/org/repo.git` → `host/org/repo`
/// - `ssh://git@host/org/repo.git` → `host/org/repo`
/// - no remote → `local/<basename>` (same key on every machine)
pub fn canonical_key(url: &str) -> String {
    let mut s = url.trim().to_string();
    // Strip scheme, then any `git@` user prefix (covers `git@host:path`
    // and `ssh://git@host/path` forms).
    for prefix in ["https://", "http://", "ssh://"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    if let Some(rest) = s.strip_prefix("git@") {
        s = rest.to_string();
    }
    // Colons always become slashes (keeps keys path-safe).
    s = s.replacen(':', "/", 1);
    // Strip trailing `.git` and slashes.
    while s.ends_with('/') {
        s.pop();
    }
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }
    s
}

/// Canonical repo key for the checkout at `dir`.
pub fn repo_key(dir: &Path) -> String {
    match git(dir, &["remote", "get-url", "origin"]) {
        Some(url) if !url.is_empty() => canonical_key(&url),
        _ => {
            let name = git(dir, &["rev-parse", "--show-toplevel"])
                .map(|t| {
                    Path::new(&t)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or(t)
                })
                .unwrap_or_else(|| {
                    dir.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                });
            format!("local/{name}")
        }
    }
}

/// Branch slug: `git rev-parse --abbrev-ref HEAD` with `/` → `--`.
pub fn branch_slug(dir: &Path) -> String {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "none".to_string())
        .replace('/', "--")
}

/// Worktree slot, per the skill rule: basename of the git common dir
/// (the `.git` worktree dir name), or basename of cwd when not in a repo.
pub fn worktree_slot(dir: &Path) -> String {
    match git(dir, &["rev-parse", "--git-common-dir"]) {
        Some(common_dir) => basename(Path::new(&common_dir)),
        None => basename(dir),
    }
}

fn basename(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string_lossy().to_string())
}

/// Collect all facts for the checkout containing `dir`.
pub fn collect_facts(dir: &Path) -> Facts {
    Facts {
        repo_key: repo_key(dir),
        branch_slug: branch_slug(dir),
        worktree_slot: worktree_slot(dir),
    }
}

// ---------------------------------------------------------------------------
// Scope routing
// ---------------------------------------------------------------------------

/// Knowledge routing scope (from the incitaciones whisper skill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// True for every repo and project: `rules.md`.
    Global,
    /// Repo-wide infra facts: `repos/<key>/env.md`.
    Repo,
    /// Branch-specific notes: `repos/<key>/branches/<slug>/notes.md`.
    Branch,
    /// Worktree-local setup: `repos/<key>/worktrees/<slot>/env.md`.
    Worktree,
    /// Shared repo-env knowledge in the active group's workspace.
    Group,
}

impl std::str::FromStr for Scope {
    type Err = WhisperError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "global" => Ok(Scope::Global),
            "repo" => Ok(Scope::Repo),
            "branch" => Ok(Scope::Branch),
            "worktree" => Ok(Scope::Worktree),
            "group" => Ok(Scope::Group),
            other => Err(WhisperError::new(format!("unknown scope '{other}'"))
                .with_suggestion("valid scopes: global | repo | branch | worktree | group")),
        }
    }
}

/// A resolved write destination.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub scope: Scope,
    pub path: PathBuf,
}

impl Target {
    /// Create the file and its parents if missing; return whether it was created.
    pub fn ensure(&self) -> std::io::Result<bool> {
        if self.path.exists() {
            return Ok(false);
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(&self.path)?;
        Ok(true)
    }

    /// Append text verbatim, normalizing to a single trailing newline.
    pub fn append(&self, text: &str) -> std::io::Result<()> {
        self.ensure()?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&self.path)?;
        let trimmed = text.trim_end();
        writeln!(f, "{trimmed}")?;
        Ok(())
    }
}

/// Resolve a scope to its exact destination path.
///
/// `global` writes to the top-level workspace root; every other scope lands
/// under the knowledge root (group root when a group is active, else the
/// workspace root).
pub fn resolve(scope: Scope, facts: &Facts, resolved: &Resolved) -> Result<Target> {
    let target = match scope {
        Scope::Global => Target {
            scope,
            path: resolved.workspace_root.join("rules.md"),
        },
        Scope::Repo => Target {
            scope,
            path: resolved
                .knowledge_root()
                .join("repos")
                .join(&facts.repo_key)
                .join("env.md"),
        },
        Scope::Branch => Target {
            scope,
            path: resolved
                .knowledge_root()
                .join("repos")
                .join(&facts.repo_key)
                .join("branches")
                .join(&facts.branch_slug)
                .join("notes.md"),
        },
        Scope::Worktree => Target {
            scope,
            path: resolved
                .knowledge_root()
                .join("repos")
                .join(&facts.repo_key)
                .join("worktrees")
                .join(&facts.worktree_slot)
                .join("env.md"),
        },
        Scope::Group => {
            let (_, group_root) = resolved.group.as_ref().ok_or_else(|| {
                WhisperError::new("no group is active for this repo")
                    .with_suggestion("set `group = \"<name>\"` in .whisper/config.toml, or list this repo's canonical key in [groups.<name>].repos in the global config")
            })?;
            Target {
                scope,
                path: group_root
                    .join("repos")
                    .join(&facts.repo_key)
                    .join("env.md"),
            }
        }
    };
    Ok(target)
}

// ---------------------------------------------------------------------------
// Layout operations
// ---------------------------------------------------------------------------

/// Result of an init run.
#[derive(Debug, Serialize)]
pub struct InitReport {
    pub workspace_root: PathBuf,
    pub group: Option<String>,
    pub created: Vec<PathBuf>,
    pub existing: Vec<PathBuf>,
}

/// Create the workspace layout for this checkout.
///
/// Creates `rules.md`, the repo slot (`env.md`, branch slot with
/// `context.md` / `plan.md` / `notes.md`, worktree slot with `env.md`).
/// Never overwrites existing files.
pub fn init(facts: &Facts, resolved: &Resolved) -> Result<InitReport> {
    let mut report = InitReport {
        workspace_root: resolved.workspace_root.clone(),
        group: resolved.group.as_ref().map(|(n, _)| n.clone()),
        created: Vec::new(),
        existing: Vec::new(),
    };

    let scopes = [Scope::Global, Scope::Repo, Scope::Branch, Scope::Worktree];
    for scope in scopes {
        let target = resolve(scope, facts, resolved)?;
        if target.ensure().map_err(WhisperError::from)? {
            report.created.push(target.path);
        } else {
            report.existing.push(target.path);
        }
    }

    // Branch slot also carries context.md and plan.md per the skill layout.
    let branch_dir = resolved
        .knowledge_root()
        .join("repos")
        .join(&facts.repo_key)
        .join("branches")
        .join(&facts.branch_slug);
    for name in ["context.md", "plan.md"] {
        let path = branch_dir.join(name);
        if !path.exists() {
            std::fs::create_dir_all(&branch_dir)?;
            std::fs::File::create(&path)?;
            report.created.push(path);
        } else {
            report.existing.push(path);
        }
    }

    Ok(report)
}

/// Legacy repo-key directory variants for this repo (non-canonical dirs
/// under `repos/` that plausibly refer to the same repo).
pub fn legacy_variants(facts: &Facts, resolved: &Resolved) -> Vec<String> {
    let repos_dir = resolved.knowledge_root().join("repos");
    let Ok(entries) = std::fs::read_dir(&repos_dir) else {
        return Vec::new();
    };
    let bare_name = facts.repo_key.rsplit('/').next().unwrap_or("").to_string();
    let mut variants = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == facts.repo_key {
            continue;
        }
        // Legacy forms: bare name, owner-only, or alias-host with colon.
        let colon_form = name.contains(':');
        let base_matches =
            name == bare_name || name.rsplit('/').next().is_some_and(|b| b == bare_name);
        if colon_form || base_matches {
            variants.push(name);
        }
    }
    variants.sort();
    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_variants() {
        assert_eq!(
            canonical_key("git@cv:charly-vibes/whisper.git"),
            "cv/charly-vibes/whisper"
        );
        assert_eq!(
            canonical_key("https://github.com/u/r.git"),
            "github.com/u/r"
        );
        assert_eq!(canonical_key("http://github.com/u/r"), "github.com/u/r");
        assert_eq!(
            canonical_key("ssh://git@github.com/u/r.git"),
            "github.com/u/r"
        );
        assert_eq!(canonical_key("git@github.com:u/r.git"), "github.com/u/r");
    }

    #[test]
    fn scope_parse() {
        use std::str::FromStr;
        assert_eq!(Scope::from_str("global").unwrap(), Scope::Global);
        assert_eq!(Scope::from_str("Worktree").unwrap(), Scope::Worktree);
        assert!(Scope::from_str("galaxy").is_err());
    }

    #[test]
    fn resolve_branch_path_shape() {
        let facts = Facts {
            repo_key: "cv/charly-vibes/whisper".into(),
            branch_slug: "feature--x".into(),
            worktree_slot: "whisper".into(),
        };
        let resolved = Resolved {
            workspace_root: PathBuf::from("/tmp/ws"),
            group: None,
        };
        let t = resolve(Scope::Branch, &facts, &resolved).unwrap();
        assert_eq!(
            t.path,
            PathBuf::from("/tmp/ws/repos/cv/charly-vibes/whisper/branches/feature--x/notes.md")
        );
    }

    #[test]
    fn group_scope_without_group_is_error() {
        let facts = Facts {
            repo_key: "github.com/u/r".into(),
            branch_slug: "main".into(),
            worktree_slot: "r".into(),
        };
        let resolved = Resolved {
            workspace_root: PathBuf::from("/tmp/ws"),
            group: None,
        };
        let err = resolve(Scope::Group, &facts, &resolved).unwrap_err();
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn append_creates_then_extends() {
        let tmp = tempfile::tempdir().unwrap();
        let facts = Facts {
            repo_key: "github.com/u/r".into(),
            branch_slug: "main".into(),
            worktree_slot: "r".into(),
        };
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
        };
        let t = resolve(Scope::Repo, &facts, &resolved).unwrap();
        assert!(t.ensure().unwrap());
        t.append("first fact").unwrap();
        t.append("second fact\n\n").unwrap();
        let content = std::fs::read_to_string(&t.path).unwrap();
        assert_eq!(content, "first fact\nsecond fact\n");
        assert!(!t.ensure().unwrap());
    }
}
