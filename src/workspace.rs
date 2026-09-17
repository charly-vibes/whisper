//! Core deterministic logic: facts derivation, scope routing, layout ops.
//!
//! Everything here is a pure function of (git facts, config). The same
//! checkout must always resolve to the same paths on every machine.

use std::path::{Path, PathBuf};
use std::process::Command;

use genesis::managed_block::{BlockDef, BlockInjector, BlockRegistry, InjectResult};
use serde::{Deserialize, Serialize};

use crate::config::Resolved;
use crate::entry::{self, Entry, Item};
use crate::{Result, WhisperError};

pub const BLOCK_NAME: &str = "turu";

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

/// Git repo root for `dir` (falls back to `dir` outside a repo).
pub fn repo_root(dir: &Path) -> PathBuf {
    git(dir, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.to_path_buf())
}

/// Path of the agent-facing file that hosts the turu managed block:
/// `AGENTS.md` at the repo root (falls back to cwd outside a repo).
pub fn agents_file(dir: &Path) -> PathBuf {
    repo_root(dir).join("AGENTS.md")
}

/// Injector with the `turu` block registered.
pub fn turu_injector() -> BlockInjector {
    let mut registry = BlockRegistry::new();
    registry.register(BlockDef::with_markers(
        BLOCK_NAME,
        "<!-- TURU:START -->\n",
        "\n<!-- TURU:END -->\n",
    ));
    BlockInjector::new(registry)
}

/// Content of the turu managed block for the current checkout: the
/// deterministic routing map, so agents read paths from a file instead of
/// re-deriving them.
pub fn managed_block_content(facts: &Facts, resolved: &Resolved) -> Result<String> {
    let path_of = |scope| resolve(scope, facts, resolved).map(|t| t.path);
    let fmt = |p: Result<PathBuf>| p.map(|p| format!("`{}`", p.display()));

    Ok(format!(
        "# Whisper knowledge workspace (managed by turu — regenerate with `turu sync`)\n\
         \n\
         - Workspace root: `{}`{}\n\
         - Repo key: `{}` · Branch slug: `{}` · Worktree slot: `{}`\n\
         - Deterministic routing (resolve, never guess):\n\
           - global → {}\n\
           - repo → {}\n\
           - branch → {}\n\
           - worktree → {}\n\
         - Commands: `turu resolve <scope>` · `turu append <scope> --text ... [--topic k] [--supersedes id]` · `turu recall <scope> [--topic k] [--budget bytes]` · `turu distill <scope> --begin|--commit` · `turu bundle pack|unpack` · `turu status` · `turu doctor`\n",
        resolved.workspace_root.display(),
        resolved
            .group
            .as_ref()
            .map(|(n, _)| format!(" (group `{n}` active — shared scopes route there)"))
            .unwrap_or_default(),
        facts.repo_key,
        facts.branch_slug,
        facts.worktree_slot,
        fmt(path_of(Scope::Global)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Repo)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Branch)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Worktree)).unwrap_or_else(|_| "(unavailable)".into()),
    ))
}

/// Inject/refresh the turu managed block in the agent-facing file.
/// Returns the target path and one of `created | prepended | updated`.
pub fn agents_sync(
    repo_root: &Path,
    target_override: Option<&str>,
    facts: &Facts,
    resolved: &Resolved,
) -> Result<(PathBuf, &'static str)> {
    let path = match target_override {
        Some(f) => PathBuf::from(f),
        None => agents_file(repo_root),
    };
    let content = managed_block_content(facts, resolved)?;
    let result = turu_injector()
        .inject(&path, BLOCK_NAME, &content)
        .map_err(WhisperError::from)?;
    let label = match result {
        InjectResult::Created => "created",
        InjectResult::Prepended => "prepended",
        InjectResult::Updated => "updated",
    };
    Ok((path, label))
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

/// Deterministic scope key used in entry-id hashing, so identical text in
/// different scopes (or branches) never collides.
pub fn scope_key(scope: Scope, facts: &Facts, resolved: &Resolved) -> String {
    match scope {
        Scope::Global => "global".to_string(),
        Scope::Repo => facts.repo_key.clone(),
        Scope::Branch => format!("{}/{}", facts.repo_key, facts.branch_slug),
        Scope::Worktree => format!("{}/{}", facts.repo_key, facts.worktree_slot),
        Scope::Group => match &resolved.group {
            Some((name, _)) => format!("group:{name}/{}", facts.repo_key),
            None => facts.repo_key.clone(), // unreachable: resolve() errors first
        },
    }
}

/// Result of an entry append.
#[derive(Debug, Serialize)]
pub struct AppendReport {
    pub id: String,
    pub duplicate: bool,
    /// Id of the entry that was marked superseded (when `supersedes` was given).
    pub superseded: Option<String>,
    /// Bytes actually written (the rendered entry line, not the raw input).
    pub bytes: usize,
}

/// Append one structured entry to a scope file: idempotent by id, with
/// mechanical supersede marking. Unmanaged freeform lines are preserved
/// verbatim in their original order.
pub fn append_entry(
    target: &Target,
    scope_key: &str,
    text: &str,
    topic: Option<&str>,
    supersedes: Option<&str>,
    ts: &str,
) -> Result<AppendReport> {
    if let Some(t) = topic {
        let invalid = t.is_empty()
            || t.chars()
                .any(|c| c.is_whitespace() || c == '(' || c == ')' || c == '#');
        if invalid {
            return Err(
                WhisperError::new(format!("invalid topic '{t}'")).with_suggestion(
                    "topics are bare keys like 'infra' — no whitespace, parens, or '#'",
                ),
            );
        }
    }
    // Normalize: no trailing whitespace overall, and no line may start
    // with whitespace — continuation lines start with exactly two spaces,
    // so leading whitespace would break the ledger round-trip.
    let text: String = text
        .trim_end()
        .lines()
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    let id = entry::entry_id(scope_key, ts, &text);
    let raw = std::fs::read_to_string(&target.path).unwrap_or_default();
    let id_present = entry::parse_file(&raw)
        .iter()
        .any(|i| matches!(i, Item::Entry(e) if e.id == id));
    if id_present {
        return Ok(AppendReport {
            id,
            duplicate: true,
            superseded: None,
            bytes: 0,
        });
    }
    let mut new = Entry {
        ts: ts.to_string(),
        id: id.clone(),
        topic: topic.map(str::to_string),
        text: text.to_string(),
        supersedes: None,
        superseded_by: None,
    };
    if let Some(sup) = supersedes {
        let mut items = entry::parse_file(&raw);
        match items
            .iter_mut()
            .find(|i| matches!(i, Item::Entry(e) if e.id == sup))
        {
            Some(Item::Entry(e)) => e.superseded_by = Some(id.clone()),
            _ => {
                return Err(WhisperError::new(format!(
                    "supersedes target not found in this scope: {sup}"
                ))
                .with_suggestion("turu recall <scope> --include-superseded to list entry ids"));
            }
        }
        new.supersedes = Some(sup.to_string());
        items.push(Item::Entry(new));
        target.ensure().map_err(WhisperError::from)?;
        // Rewrite path (rare): concurrent appends during this window can be
        // lost — see design.md; the fast path below is the safe default.
        let rendered = entry::render_file(&items);
        std::fs::write(&target.path, &rendered).map_err(WhisperError::from)?;
        return Ok(AppendReport {
            id,
            duplicate: false,
            superseded: supersedes.map(str::to_string),
            bytes: rendered.len(),
        });
    }
    // Fast path: single O_APPEND write — concurrent agents appending in the
    // same window cannot lose each other's entries.
    target.ensure().map_err(WhisperError::from)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&target.path)
        .map_err(WhisperError::from)?;
    let mut line = entry::render_file(&[Item::Entry(new)]);
    if !raw.is_empty() && !raw.ends_with('\n') {
        line.insert(0, '\n');
    }
    f.write_all(line.as_bytes()).map_err(WhisperError::from)?;
    Ok(AppendReport {
        id,
        duplicate: false,
        superseded: None,
        bytes: line.len(),
    })
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
        // Legacy forms: bare name, or alias-host with a matching basename
        // (`host:name`). Colon dirs whose path doesn't end in the bare name
        // belong to other repos — consolidate must never touch them.
        let colon_form = name
            .split_once(':')
            .is_some_and(|(_, rest)| rest == bare_name);
        let base_matches = name == bare_name;
        if colon_form || base_matches {
            variants.push(name);
        }
    }
    variants.sort();
    variants
}

/// Report of a `consolidate` migration.
#[derive(Debug, Default, Serialize)]
pub struct ConsolidateReport {
    /// Legacy variant directory names that were migrated.
    pub variants: Vec<String>,
    /// Files/dirs moved into the canonical key without conflict.
    pub moved: Vec<PathBuf>,
    /// Files merged by extending (deduped lines) into existing files.
    pub merged: Vec<PathBuf>,
    /// The canonical repo directory everything landed in.
    pub canonical_dir: PathBuf,
}

/// Migrate legacy repo-key directories into the canonical key dir.
///
/// Pure deterministic migration: when the canonical dir is absent, the
/// legacy dir is renamed wholesale; otherwise each legacy entry is moved
/// (no conflict) or merged by extending existing text files without
/// duplicating lines. Legacy dirs are removed afterwards.
pub fn consolidate(facts: &Facts, resolved: &Resolved) -> Result<ConsolidateReport> {
    let repos_dir = resolved.knowledge_root().join("repos");
    let canonical_dir = repos_dir.join(&facts.repo_key);
    let variants = legacy_variants(facts, resolved);
    let mut report = ConsolidateReport {
        variants: variants.clone(),
        canonical_dir,
        ..ConsolidateReport::default()
    };

    for variant in &variants {
        let src = repos_dir.join(variant);
        if !report.canonical_dir.exists() {
            std::fs::create_dir_all(
                report
                    .canonical_dir
                    .parent()
                    .ok_or_else(|| WhisperError::new("repos dir has no parent"))?,
            )?;
            std::fs::rename(&src, &report.canonical_dir)?;
            report.moved.push(report.canonical_dir.clone());
        } else {
            merge_dir(
                &src,
                &report.canonical_dir,
                &mut report.moved,
                &mut report.merged,
            )?;
            std::fs::remove_dir_all(&src)?;
        }
    }
    Ok(report)
}

/// Move/merge every entry of `src` into `dst`, then `src` must be empty.
fn merge_dir(
    src: &Path,
    dst: &Path,
    moved: &mut Vec<PathBuf>,
    merged: &mut Vec<PathBuf>,
) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            merge_dir(&from, &to, moved, merged)?;
        } else if !to.exists() {
            std::fs::rename(&from, &to)?;
            moved.push(to);
        } else {
            merge_text_file(&from, &to)?;
            merged.push(to);
        }
    }
    Ok(())
}

/// Extend `to` with the lines of `from` that are not already present.
fn merge_text_file(from: &Path, to: &Path) -> Result<()> {
    let dst = std::fs::read_to_string(to)?;
    let src = std::fs::read_to_string(from)?;
    let existing: std::collections::HashSet<&str> = dst.lines().map(str::trim_end).collect();
    let addition: Vec<&str> = src
        .lines()
        .filter(|l| !l.trim_end().is_empty() && !existing.contains(l.trim_end()))
        .collect();
    if !addition.is_empty() {
        let mut out = dst.clone();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&addition.join("\n"));
        out.push('\n');
        std::fs::write(to, out)?;
    }
    Ok(())
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
            shadowed_global_root: None,
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
            shadowed_global_root: None,
        };
        let err = resolve(Scope::Group, &facts, &resolved).unwrap_err();
        assert!(err.suggestion.is_some());
    }

    fn ws_facts(repo_key: &str) -> Facts {
        Facts {
            repo_key: repo_key.into(),
            branch_slug: "main".into(),
            worktree_slot: "ws".into(),
        }
    }

    #[test]
    fn legacy_variants_flags_bare_and_matching_colon_forms_only() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["whisper", "host:whisper", "ak:akielbowicz", "github.com"] {
            std::fs::create_dir_all(tmp.path().join("repos").join(name)).unwrap();
        }
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
        };

        let variants = legacy_variants(&ws_facts("github.com/u/whisper"), &resolved);

        assert_eq!(
            variants,
            vec!["host:whisper".to_string(), "whisper".to_string()]
        );
    }

    #[test]
    fn consolidate_moves_whole_dir_when_canonical_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let repos = tmp.path().join("repos");
        let legacy = repos.join("whisper");
        std::fs::create_dir_all(legacy.join("branches/main")).unwrap();
        std::fs::write(legacy.join("env.md"), "legacy fact\n").unwrap();
        std::fs::write(legacy.join("branches/main/notes.md"), "note\n").unwrap();
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
        };

        let report = consolidate(&ws_facts("github.com/u/whisper"), &resolved).unwrap();

        assert_eq!(report.variants, vec!["whisper".to_string()]);
        let canon = repos.join("github.com/u/whisper");
        assert_eq!(
            std::fs::read_to_string(canon.join("env.md")).unwrap(),
            "legacy fact\n"
        );
        assert_eq!(
            std::fs::read_to_string(canon.join("branches/main/notes.md")).unwrap(),
            "note\n"
        );
        assert!(!legacy.exists());
    }

    #[test]
    fn consolidate_merges_extending_never_duplicating() {
        let tmp = tempfile::tempdir().unwrap();
        let repos = tmp.path().join("repos");
        let canon = repos.join("github.com/u/whisper");
        let legacy = repos.join("whisper");
        std::fs::create_dir_all(&canon).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(canon.join("env.md"), "shared fact\ncanonical only\n").unwrap();
        std::fs::write(legacy.join("env.md"), "legacy only\nshared fact\n\n").unwrap();
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
        };

        let report = consolidate(&ws_facts("github.com/u/whisper"), &resolved).unwrap();

        let merged = std::fs::read_to_string(canon.join("env.md")).unwrap();
        assert_eq!(merged, "shared fact\ncanonical only\nlegacy only\n");
        assert_eq!(report.merged, vec![canon.join("env.md")]);
        assert!(!legacy.exists());
    }

    #[test]
    fn consolidate_merges_nested_files_into_existing_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let repos = tmp.path().join("repos");
        let canon = repos.join("github.com/u/whisper");
        let legacy = repos.join("whisper");
        std::fs::create_dir_all(canon.join("branches/main")).unwrap();
        std::fs::create_dir_all(legacy.join("branches/main")).unwrap();
        std::fs::create_dir_all(legacy.join("branches/feature--x")).unwrap();
        std::fs::write(canon.join("branches/main/notes.md"), "shared note\n").unwrap();
        std::fs::write(
            legacy.join("branches/main/notes.md"),
            "shared note\nlegacy note\n",
        )
        .unwrap();
        std::fs::write(
            legacy.join("branches/feature--x/notes.md"),
            "feature note\n",
        )
        .unwrap();
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
        };

        consolidate(&ws_facts("github.com/u/whisper"), &resolved).unwrap();

        assert_eq!(
            std::fs::read_to_string(canon.join("branches/main/notes.md")).unwrap(),
            "shared note\nlegacy note\n"
        );
        assert_eq!(
            std::fs::read_to_string(canon.join("branches/feature--x/notes.md")).unwrap(),
            "feature note\n"
        );
        assert!(!legacy.exists());
    }

    #[test]
    fn consolidate_noop_without_variants() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
        };

        let report = consolidate(&ws_facts("github.com/u/r"), &resolved).unwrap();

        assert!(report.variants.is_empty());
        assert!(report.moved.is_empty());
        assert!(report.merged.is_empty());
        assert!(!tmp.path().join("repos").exists());
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
            shadowed_global_root: None,
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
