//! Workspace provisioning: init, legacy-key consolidation, and scope
//! file enumeration.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{Resolved, Routing};
use crate::{Result, WhisperError};

use super::resolve::ensure_private_ignored;
use super::{Facts, Scope, Target, is_in_private_zone, resolve};

pub(crate) fn global_store_scopes(facts: &Facts) -> Vec<Scope> {
    if facts.repo_local_root.is_some() {
        vec![Scope::Repo, Scope::Branch]
    } else {
        Vec::new()
    }
}

/// Machine-local store destination for `repo`/`branch` scopes (the `--global`
/// escape hatch; the default destination when there is no checkout). Panics
/// only on programmer error — these two scopes always resolve.
/// Repo/branch scope destinations (gh-3): the checkout's committed zone
/// under `repo-local` routing, the machine-local store otherwise — the
/// persistent `--global` equivalent. Group routing (when active) and the
/// `--global`/`--private` per-invocation flags compose on top and keep
/// priority.
pub(super) fn repo_branch_target(scope: Scope, facts: &Facts, resolved: &Resolved) -> Target {
    let target = |path: PathBuf| -> Target { Target { scope, path } };
    let root = resolved
        .knowledge_root()
        .join("repos")
        .join(&facts.repo_key);
    if let Some(checkout) = facts
        .repo_local_root
        .as_ref()
        .filter(|_| resolved.routing == Routing::RepoLocal)
    {
        return target(match scope {
            Scope::Repo => checkout.join("env.md"),
            _ => checkout
                .join("branches")
                .join(&facts.branch_slug)
                .join("notes.md"),
        });
    }
    target(match scope {
        Scope::Repo => root.join("env.md"),
        _ => root
            .join("branches")
            .join(&facts.branch_slug)
            .join("notes.md"),
    })
}

pub(super) fn global_store_target(scope: Scope, facts: &Facts, resolved: &Resolved) -> Target {
    assert!(
        matches!(scope, Scope::Repo | Scope::Branch),
        "global-store escape hatch is defined for repo/branch scopes only"
    );
    let mut f = facts.clone();
    f.repo_local_root = None;
    resolve(scope, &f, resolved).expect("repo/branch resolve without a checkout")
}

/// Existing scope files for doctor's per-file sweeps: every distinct scope
/// destination, checkout layer first. Skips a checkout layer identical to
/// the store layer (no repo-local root) and a store layer relocated onto
/// the checkout by a repo-private `workspace_root` override.
pub fn scope_files_for(facts: &Facts, resolved: &Resolved) -> Vec<PathBuf> {
    scope_files_for_scoped(facts, resolved)
        .into_iter()
        .map(|(_, p)| p)
        .collect()
}

/// Same as [`scope_files_for`], keeping each file's routing scope.
pub fn scope_files_for_scoped(facts: &Facts, resolved: &Resolved) -> Vec<(Scope, PathBuf)> {
    let mut files = Vec::new();
    for scope in [Scope::Global, Scope::Repo, Scope::Branch, Scope::Worktree] {
        if let Ok(t) = resolve(scope, facts, resolved) {
            files.push((scope, t.path));
        }
        if matches!(scope, Scope::Repo | Scope::Branch) && facts.repo_local_root.is_some() {
            let t = global_store_target(scope, facts, resolved);
            if !files[1..].iter().any(|(_, p)| *p == t.path) {
                files.push((scope, t.path));
            }
        }
    }
    files
}

/// Create the workspace layout for this checkout.
///
/// Creates `rules.md`, the repo slot (`env.md`, branch slot with
/// `context.md` / `plan.md` / `notes.md`, worktree slot with `env.md`),
/// plus the machine-local store slots for `repo`/`branch` (the `--global`
/// escape hatch and the recall-composition fallback). Never overwrites
/// existing files.
pub fn init(facts: &Facts, resolved: &Resolved) -> Result<InitReport> {
    let mut report = InitReport {
        workspace_root: resolved.workspace_root.clone(),
        group: resolved.group.as_ref().map(|(n, _)| n.clone()),
        created: Vec::new(),
        existing: Vec::new(),
        ignore_rule_added: false,
    };

    // Checkout self-protection (3.1): the private zone must be effectively
    // ignored before any `--private` append can put knowledge there.
    report.ignore_rule_added = ensure_private_ignored(facts)?;
    provision_slots(facts, resolved, &mut report)?;
    provision_branch_files(facts, resolved, &mut report)?;
    Ok(report)
}

/// Provision the four base slots plus the machine-local store layer
/// (skipped under machine-local routing, where resolve() already
/// provisions it — no scope gets two provisioning entries).
fn provision_slots(facts: &Facts, resolved: &Resolved, report: &mut InitReport) -> Result<()> {
    let scopes = [Scope::Global, Scope::Repo, Scope::Branch, Scope::Worktree];
    for scope in scopes {
        let target = resolve(scope, facts, resolved)?;
        record_ensure(target, report)?;
    }
    let store_scopes = match resolved.routing {
        Routing::RepoLocal => global_store_scopes(facts),
        Routing::MachineLocal => Vec::new(),
    };
    for scope in store_scopes {
        let target = global_store_target(scope, facts, resolved);
        record_ensure(target, report)?;
    }
    Ok(())
}

/// Ensure a target exists and record it as created or existing.
fn record_ensure(target: Target, report: &mut InitReport) -> Result<()> {
    if target.ensure().map_err(WhisperError::from)? {
        report.created.push(target.path);
    } else {
        report.existing.push(target.path);
    }
    Ok(())
}

/// Branch slot also carries context.md and plan.md per the skill layout;
/// it follows the branch scope's resolved home (checkout or store).
fn provision_branch_files(
    facts: &Facts,
    resolved: &Resolved,
    report: &mut InitReport,
) -> Result<()> {
    let branch_dir = resolve(Scope::Branch, facts, resolved)?
        .path
        .parent()
        .expect("branch target always has a parent (the branch dir)")
        .to_path_buf();
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
    Ok(())
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
    guard_private_zone(&canonical_dir, facts)?;
    let variants = legacy_variants(facts, resolved);
    let mut report = ConsolidateReport {
        variants: variants.clone(),
        canonical_dir,
        ..ConsolidateReport::default()
    };
    fold_variants(&repos_dir, &variants, &mut report)?;
    Ok(report)
}

/// Regression guard (add-repo-private-scope 2.6): the private zone is
/// never an input to a move or merge.
fn guard_private_zone(canonical_dir: &Path, facts: &Facts) -> Result<()> {
    if is_in_private_zone(canonical_dir, facts) {
        return Err(WhisperError::new(
            "refusing to consolidate: the canonical repo dir lies under the checkout's private zone",
        )
        .with_suggestion("the private zone is never moved, merged, or removed by any verb"));
    }
    Ok(())
}

/// Fold each legacy variant dir into the canonical dir (rename when the
/// canonical dir is new, merge when it already exists).
fn fold_variants(
    repos_dir: &Path,
    variants: &[String],
    report: &mut ConsolidateReport,
) -> Result<()> {
    for variant in variants {
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
    Ok(())
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

/// Result of an init run.
#[derive(Debug, Serialize)]
pub struct InitReport {
    pub workspace_root: PathBuf,
    pub group: Option<String>,
    pub created: Vec<PathBuf>,
    pub existing: Vec<PathBuf>,
    /// Whether init appended the private-zone ignore rule to the root
    /// `.gitignore` (false when already effectively ignored or outside a
    /// checkout).
    pub ignore_rule_added: bool,
}
