//! Core deterministic logic: facts derivation, scope routing, layout ops.
//!
//! Everything here is a pure function of (git facts, config). The same
//! checkout must always resolve to the same paths on every machine. The
//! private zone (`.whisper/private/`) is one of those pure resolutions:
//! a path, not a policy — defined only for repo/branch scopes inside a
//! checkout, and written only by explicit private-scope operations.
//! The zone's gitignore state is likewise deterministic: `init`/`sync`
//! repair it against `git check-ignore` truth, never by text matching.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

mod block;
mod init;
mod resolve;
mod scope;

pub use block::{agents_sync, managed_block_content, turu_injector};
pub use init::{
    ConsolidateReport, InitReport, consolidate, init, legacy_variants, scope_files_for,
    scope_files_for_scoped,
};
pub use resolve::ensure_private_ignored;
pub use resolve::{
    PrivateZoneState, is_in_private_zone, private_zone_root, private_zone_state,
    private_zone_target, resolve, tracked_files_under_private_zone,
};
pub use scope::{AppendReport, Scope, Target, append_entry, scope_key};

pub const BLOCK_NAME: &str = "turu";

/// Directory name of the repo-local knowledge root inside a checkout
/// (decision A of the transport fork, add-repo-private-scope).
pub const REPO_LOCAL_DIR: &str = ".whisper";

/// Deterministic facts about the current checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facts {
    /// Canonical repo key, e.g. `github.com/u/r`, `cv/org/repo`, `local/name`.
    pub repo_key: String,
    /// Branch slug: `feature/x` → `feature--x`.
    pub branch_slug: String,
    /// Worktree slot: basename of the `.git` common dir, or of cwd.
    pub worktree_slot: String,
    /// Repo-local knowledge root: `<repo-root>/.whisper` when the facts were
    /// collected inside a git repo, `None` outside one. `repo`/`branch`
    /// scopes resolve here so knowledge travels with the repo's own git
    /// (decision A, add-repo-private-scope).
    pub repo_local_root: Option<PathBuf>,
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
    let trimmed = url.trim();
    // Strip scheme, then any `git@` user prefix (covers `git@host:path`
    // and `ssh://git@host/path` forms).
    let stripped = ["https://", "http://", "ssh://"]
        .iter()
        .find_map(|p| trimmed.strip_prefix(p))
        .unwrap_or(trimmed);
    let s = stripped.strip_prefix("git@").unwrap_or(stripped);
    // Colons always become slashes (keeps keys path-safe).
    let s = s.replacen(':', "/", 1);
    // Strip trailing `.git` and slashes.
    let s = s.strip_suffix(".git").unwrap_or(&s);
    s.trim_end_matches('/').to_string()
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
/// In a bare repo the common dir is the bare dir itself, which git reports
/// as `.` — the slot is then the bare dir's basename (deployed scenario
/// workspace-routing/bare-repo).
pub fn worktree_slot(dir: &Path) -> String {
    match git(dir, &["rev-parse", "--git-common-dir"]) {
        Some(common_dir) if common_dir == "." => basename(dir),
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

pub fn collect_facts(dir: &Path) -> Facts {
    // The repo-local root exists only for a genuine working tree — outside
    // one, repo/branch scopes fall back to the machine-local store.
    let in_repo = git(dir, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|v| v == "true");
    Facts {
        repo_key: repo_key(dir),
        branch_slug: branch_slug(dir),
        worktree_slot: worktree_slot(dir),
        repo_local_root: if in_repo {
            Some(repo_root(dir).join(REPO_LOCAL_DIR))
        } else {
            None
        },
    }
}

// ---------------------------------------------------------------------------
// Scope routing
// ---------------------------------------------------------------------------

#[cfg(test)]
mod uw {
    //! Shared test helpers: unwrap trait, fixtures, fact builders.

    use std::path::PathBuf;

    use crate::config::{Resolved, Routing};

    use super::Facts;

    pub trait Uw<T> {
        fn uw(self) -> T;
    }
    impl<T, E: std::fmt::Debug> Uw<T> for std::result::Result<T, E> {
        fn uw(self) -> T {
            match self {
                Ok(v) => v,
                Err(e) => panic!("unexpected error: {e:?}"),
            }
        }
    }
    impl<T> Uw<T> for Option<T> {
        fn uw(self) -> T {
            match self {
                Some(v) => v,
                None => panic!("unexpected None"),
            }
        }
    }

    /// (store tmp dir, checkout tmp dir, checkout facts, workspace resolution).
    pub(super) fn checkout_fixture() -> (tempfile::TempDir, tempfile::TempDir, Facts, Resolved) {
        let tmp = tempfile::tempdir().uw();
        let checkout = tempfile::tempdir().uw();
        let facts = Facts {
            repo_key: "github.com/u/r".into(),
            branch_slug: "main".into(),
            worktree_slot: "r".into(),
            repo_local_root: Some(checkout.path().join(".whisper")),
        };
        let resolved = Resolved {
            workspace_root: tmp.path().to_path_buf(),
            group: None,
            shadowed_global_root: None,
            routing: Routing::RepoLocal,
        };
        (tmp, checkout, facts, resolved)
    }

    pub(super) fn checkout_path_facts() -> Facts {
        Facts {
            repo_key: "github.com/u/r".into(),
            branch_slug: "main".into(),
            worktree_slot: ".git".into(),
            repo_local_root: Some(PathBuf::from("/checkout/.whisper")),
        }
    }

    pub(super) fn ws_facts(repo_key: &str) -> Facts {
        Facts {
            repo_key: repo_key.into(),
            branch_slug: "main".into(),
            worktree_slot: "ws".into(),
            repo_local_root: None,
        }
    }
}

#[cfg(test)]
mod layout;
#[cfg(test)]
mod tests;
