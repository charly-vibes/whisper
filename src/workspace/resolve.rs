//! Scope resolution and the private-zone integrity surface.

use std::path::{Path, PathBuf};

use crate::config::Resolved;
use crate::{Result, WhisperError};

use super::init::repo_branch_target;
use super::{Facts, REPO_LOCAL_DIR, Scope, Target};

pub fn resolve(scope: Scope, facts: &Facts, resolved: &Resolved) -> Result<Target> {
    let path = match scope {
        Scope::Global => resolved.workspace_root.join("rules.md"),
        Scope::Repo | Scope::Branch => return Ok(repo_branch_target(scope, facts, resolved)),
        Scope::Worktree => worktree_path(facts, resolved),
        Scope::Group => group_path(scope, facts, resolved)?,
    };
    Ok(Target { scope, path })
}

/// Worktree knowledge is machine-local by definition: under the knowledge
/// root, keyed by repo and worktree slot.
fn worktree_path(facts: &Facts, resolved: &Resolved) -> PathBuf {
    resolved
        .knowledge_root()
        .join("repos")
        .join(&facts.repo_key)
        .join("worktrees")
        .join(&facts.worktree_slot)
        .join("env.md")
}

/// Group scope addresses the shared workspace; an active group is required.
fn group_path(_scope: Scope, facts: &Facts, resolved: &Resolved) -> Result<PathBuf> {
    let (_, group_root) = resolved.group.as_ref().ok_or_else(|| {
        WhisperError::new("no group is active for this repo")
            .with_suggestion("set `group = \"<name>\"` in .whisper/config.toml, or list this repo's canonical key in [groups.<name>].repos in the global config")
    })?;
    Ok(group_root
        .join("repos")
        .join(&facts.repo_key)
        .join("env.md"))
}

/// Private-zone destination for `repo`/`branch` scopes: the repo-local
/// layout mirrored under `<checkout>/.whisper/private/` (gitignored, never
/// pushed — privacy is a path, not a policy). Defined only for those two
/// scopes and only inside a checkout: the private zone is a checkout
/// concept, and no verb writes here except explicit private routing.
pub fn private_zone_target(scope: Scope, facts: &Facts, _resolved: &Resolved) -> Result<Target> {
    if !matches!(scope, Scope::Repo | Scope::Branch) {
        return Err(
            WhisperError::new("--private applies only to the repo and branch scopes")
                .with_suggestion(match scope {
                    Scope::Global => "global always writes to the workspace root — drop --private",
                    Scope::Worktree => {
                        "worktree always writes to the workspace root — drop --private"
                    }
                    Scope::Group => "group always writes to the group root — drop --private",
                    _ => unreachable!("scope gate above"),
                }),
        );
    }
    let root = private_zone_root(facts).ok_or_else(|| {
        WhisperError::new("--private requires a git checkout")
            .with_suggestion("the private zone lives in <checkout>/.whisper/private/ — use --global to write the machine-local store instead")
    })?;
    let path = match scope {
        Scope::Repo => root.join("env.md"),
        Scope::Branch => root
            .join("branches")
            .join(&facts.branch_slug)
            .join("notes.md"),
        _ => unreachable!("scope gate above"),
    };
    Ok(Target { scope, path })
}

/// The checkout's private zone root: the exact top-level path
/// `.whisper/private/`. `None` outside a checkout — the zone is a checkout
/// concept.
pub fn private_zone_root(facts: &Facts) -> Option<PathBuf> {
    facts
        .repo_local_root
        .as_ref()
        .map(|root| root.join("private"))
}

/// True when `path` lies under the checkout's private zone. Component-wise
/// prefix match: `.whisper/privateX/` and a `private/` nested deeper do NOT
/// inherit the guarantee — the boundary is the exact top-level path.
pub fn is_in_private_zone(path: &Path, facts: &Facts) -> bool {
    private_zone_root(facts).is_some_and(|zone| path.starts_with(zone))
}

// ---------------------------------------------------------------------------
// Checkout self-protection (add-repo-private-scope 3.1)
// ---------------------------------------------------------------------------

/// Effective-ignore state of the checkout's private zone.
#[derive(Debug, PartialEq, Eq)]
pub enum PrivateZoneState {
    /// No checkout — the zone resolves nowhere.
    NoCheckout,
    /// `git check-ignore` reports the zone as ignored.
    Ignored,
    /// Zone reachable by git — needs the ignore rule.
    Exposed,
}

/// Effective-ignore state of the checkout's private zone: `git
/// check-ignore` is the truth, not text presence — a nested `.gitignore`
/// (or the user's global excludes file) can already carry the rule. The
/// probe is a synthetic file path *under* the zone: a dir-only pattern
/// (`private/`) does not match the bare directory name before the
/// directory exists, but it must match every file the zone will contain.
pub fn private_zone_state(facts: &Facts) -> PrivateZoneState {
    let Some(local_root) = &facts.repo_local_root else {
        return PrivateZoneState::NoCheckout;
    };
    let checkout = local_root
        .parent()
        .expect("repo-local root always has a parent (the checkout)");
    let probe = checkout.join(REPO_LOCAL_DIR).join("private").join("probe");
    // Fail-closed at THIS call site (whisper-tlp): genesis::git::is_ignored
    // distinguishes not-ignored from git failure; a failure cannot prove
    // the zone safe, so it maps to Exposed and the repair path runs.
    match genesis::git::is_ignored(checkout, &probe) {
        Ok(true) => PrivateZoneState::Ignored,
        Ok(false) | Err(_) => PrivateZoneState::Exposed,
    }
}

/// Files tracked by git under the checkout's private zone. Tracked files
/// mean the knowledge is already in history — git history is forever,
/// so doctor reports this as a prominent failure, never a warning.
pub fn tracked_files_under_private_zone(facts: &Facts) -> Vec<PathBuf> {
    let Some(local_root) = &facts.repo_local_root else {
        return Vec::new();
    };
    let checkout = local_root
        .parent()
        .expect("repo-local root always has a parent (the checkout)");
    let zone = checkout.join(REPO_LOCAL_DIR).join("private");
    let mut found = Vec::new();
    walk_files(&zone, &mut found);
    found.retain(|path| {
        // Lossy like the pre-port `ls-files` sweep: a file git cannot query
        // is skipped, never reported as a leak. Ordering is deterministic
        // (walk_files sorts per directory), matching doctor output needs.
        genesis::git::tracked(checkout, path).unwrap_or(false)
    });
    found
}

/// Files under `dir`, depth-first, sorted within each directory for
/// deterministic enumeration order.
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Ensure the checkout's private zone is effectively ignored by git.
///
/// Appends `.whisper/private/` to the root `.gitignore` only when
/// `git check-ignore` does not already report the zone as ignored — never
/// duplicates the rule, never touches other entries. A no-op outside a
/// checkout (no repo-local root → no zone to protect). Returns whether
/// the rule was appended.
pub fn ensure_private_ignored(facts: &Facts) -> Result<bool> {
    if private_zone_state(facts) != PrivateZoneState::Exposed {
        return Ok(false);
    }
    let checkout = facts
        .repo_local_root
        .as_ref()
        .expect("Exposed implies a checkout")
        .parent()
        .expect("repo-local root always has a parent (the checkout)")
        .to_path_buf();
    let rel = Path::new(REPO_LOCAL_DIR).join("private");
    let gitignore = checkout.join(".gitignore");
    let mut content = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!(
        "# turu private zone — machine-specific knowledge, never pushed (git history is forever)\n{}/\n",
        rel.to_string_lossy()
    ));
    std::fs::write(&gitignore, content)?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Layout operations
// ---------------------------------------------------------------------------
