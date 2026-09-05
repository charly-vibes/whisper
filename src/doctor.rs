//! Deep workspace diagnostics, following the genesis doctor conventions.
//!
//! Checks are assembled directly as `CheckEntry`s (config- and git-aware,
//! so the `DoctorCheck` trait's `repo_root`-only signature doesn't fit).

use std::path::Path;

use genesis::doctor::{CheckEntry, DoctorReport};

use crate::config::Resolved;
use crate::workspace::{Facts, Scope, agents_file, legacy_variants, resolve, turu_injector};

const REPO_SLOT: &str = "turu.repo-slot";
const RULES: &str = "turu.rules-md";
const BRANCH_SLOT: &str = "turu.branch-slot";
const LEGACY_KEYS: &str = "turu.legacy-keys";
const GROUP_ROOT: &str = "turu.group-root";
const MANAGED_BLOCK: &str = "turu.managed-block";

/// Run all doctor checks for the current checkout.
pub fn run_checks(facts: &Facts, resolved: &Resolved, repo_root: &Path) -> DoctorReport {
    let mut checks = Vec::new();

    // Global rules file.
    let rules = resolved.workspace_root.join("rules.md");
    checks.push(if rules.exists() {
        CheckEntry::pass(
            RULES,
            "global rules file exists at the workspace root",
            format!("`{}` present", rules.display()),
        )
    } else {
        with_fix(
            CheckEntry::warn(
                RULES,
                "global rules file exists at the workspace root",
                format!("missing at `{}`", rules.display()),
            ),
            "turu init",
        )
    });

    // Repo slot.
    let repo_env = resolve(Scope::Repo, facts, resolved).map(|t| t.path);
    checks.push(match repo_env {
        Ok(p) if p.exists() => CheckEntry::pass(
            REPO_SLOT,
            "canonical repo slot exists in the workspace",
            format!("`{}` present", p.display()),
        ),
        Ok(p) => with_fix(
            CheckEntry::warn(
                REPO_SLOT,
                "canonical repo slot exists in the workspace",
                format!("missing at `{}`", p.display()),
            ),
            "turu init",
        ),
        Err(e) => CheckEntry::fail(
            REPO_SLOT,
            "canonical repo slot exists in the workspace",
            e.message,
            None,
        ),
    });

    // Branch slot.
    let notes = resolve(Scope::Branch, facts, resolved).map(|t| t.path);
    checks.push(match notes {
        Ok(p) if p.exists() => CheckEntry::pass(
            BRANCH_SLOT,
            "branch slot exists for the current branch",
            format!("`{}` present", p.display()),
        ),
        Ok(p) => with_fix(
            CheckEntry::warn(
                BRANCH_SLOT,
                "branch slot exists for the current branch",
                format!("missing at `{}`", p.display()),
            ),
            "turu init",
        ),
        Err(e) => CheckEntry::fail(
            BRANCH_SLOT,
            "branch slot exists for the current branch",
            e.message,
            None,
        ),
    });

    // Legacy repo-key variants.
    let variants = legacy_variants(facts, resolved);
    checks.push(if variants.is_empty() {
        CheckEntry::pass(
            LEGACY_KEYS,
            "no legacy repo-key directory variants",
            format!(
                "canonical key `{}` has no competing variants",
                facts.repo_key
            ),
        )
    } else {
        with_fix(
            CheckEntry::warn(
                LEGACY_KEYS,
                "no legacy repo-key directory variants",
                format!(
                    "variants found under repos/: [{}] — canonical key is `{}`",
                    variants.join(", "),
                    facts.repo_key
                ),
            ),
            "turu consolidate (pending) — migrate manually for now",
        )
    });

    // Group root, when a group is active.
    if let Some((name, root)) = &resolved.group {
        checks.push(if root.exists() {
            CheckEntry::pass(
                GROUP_ROOT,
                "active group workspace directory exists",
                format!("group `{name}` at `{}`", root.display()),
            )
        } else {
            with_fix(
                CheckEntry::warn(
                    GROUP_ROOT,
                    "active group workspace directory exists",
                    format!("group `{name}` root missing at `{}`", root.display()),
                ),
                "turu init",
            )
        });
    }

    // Managed block in the agent-facing file.
    let agents = agents_file(repo_root);
    let has_block = agents.exists()
        && turu_injector().registry().get("turu").is_some_and(|_| {
            std::fs::read_to_string(&agents)
                .map(|raw| raw.contains("<!-- TURU:START -->"))
                .unwrap_or(false)
        });
    checks.push(if has_block {
        CheckEntry::pass(
            MANAGED_BLOCK,
            "turu managed block present in AGENTS.md",
            format!("`{}` carries the block", agents.display()),
        )
    } else {
        with_fix(
            CheckEntry::warn(
                MANAGED_BLOCK,
                "turu managed block present in AGENTS.md",
                format!("`{}` has no `<!-- TURU:START -->` block", agents.display()),
            ),
            "turu sync",
        )
    });

    DoctorReport::new("turu", checks)
}

fn with_fix(mut entry: CheckEntry, fix: &str) -> CheckEntry {
    entry.fix = Some(fix.to_string());
    entry
}
