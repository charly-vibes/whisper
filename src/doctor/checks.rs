//! Doctor check assembly: slot/layout, private trio, and managed-file
//! checks. Knowledge-content checks live in `content.rs`; the routing
//! verdict lives in `routing.rs`; the leak lint lives in `leaks.rs`.

use std::path::Path;

use genesis::doctor::{CheckEntry, DoctorReport};

use super::{
    BRANCH_SLOT, LEGACY_KEYS, PRIVATE_IGNORE, PRIVATE_LEAKS, PRIVATE_TRACKED, REPO_SLOT, RULES,
};
use crate::config::Resolved;
use crate::workspace::{
    Facts, PrivateZoneState, Scope, legacy_variants, private_zone_state, resolve,
    tracked_files_under_private_zone,
};

use super::content::content_checks;
use super::leaks::leak_warnings;
use super::managed::{
    check_group_root, check_managed_block, check_managed_skills, check_shadowed_global,
};
use super::routing::routing_check;

pub fn run_checks(facts: &Facts, resolved: &Resolved, repo_root: &Path) -> DoctorReport {
    let mut checks = slot_checks(facts, resolved);
    checks.extend(content_checks(facts, resolved));
    checks.push(routing_check(facts, resolved));
    checks.extend(private_checks(facts, resolved));
    checks.push(check_managed_block(repo_root));
    checks.push(check_managed_skills(repo_root));
    DoctorReport::new("turu", checks)
}

/// Slot/layout checks: rules file, repo/branch slots, legacy keys, group.
fn slot_checks(facts: &Facts, resolved: &Resolved) -> Vec<CheckEntry> {
    let mut checks = vec![
        check_rules(resolved),
        check_repo_slot(facts, resolved),
        check_branch_slot(facts, resolved),
        check_legacy_keys(facts, resolved),
    ];
    checks.extend(check_group_root(resolved));
    checks.extend(check_shadowed_global(resolved));
    checks
}

/// Private-zone integrity trio: effective ignore, tracked files, leaks.
fn private_checks(facts: &Facts, resolved: &Resolved) -> Vec<CheckEntry> {
    vec![
        check_private_ignore(facts),
        check_private_tracked(facts),
        check_private_leaks(facts, resolved),
    ]
}

// Global rules file.
fn check_rules(resolved: &Resolved) -> CheckEntry {
    // Global rules file.
    let rules = resolved.workspace_root.join("rules.md");
    if rules.exists() {
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
    }
}

// Repo slot.
fn check_repo_slot(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Repo slot.
    let repo_env = resolve(Scope::Repo, facts, resolved).map(|t| t.path);
    match repo_env {
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
    }
}

// Branch slot.
fn check_branch_slot(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Branch slot.
    let notes = resolve(Scope::Branch, facts, resolved).map(|t| t.path);
    match notes {
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
    }
}

// Legacy repo-key variants.
fn check_legacy_keys(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Legacy repo-key variants.
    let variants = legacy_variants(facts, resolved);
    if variants.is_empty() {
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
            "turu consolidate",
        )
    }
}

// Private-zone integrity (add-repo-private-scope 3.2): the checkout
// self-protects and self-reports. Gitignore is the backstop, turu is
// the guarantee — doctor owns the hard guarantee.
fn check_private_ignore(facts: &Facts) -> CheckEntry {
    // Private-zone integrity (add-repo-private-scope 3.2): the checkout
    // self-protects and self-reports. Gitignore is the backstop, turu is
    // the guarantee — doctor owns the hard guarantee.
    match private_zone_state(facts) {
        PrivateZoneState::NoCheckout => CheckEntry::pass(
            PRIVATE_IGNORE,
            "private zone is effectively ignored",
            "no checkout in play — the private zone resolves nowhere",
        ),
        PrivateZoneState::Ignored => CheckEntry::pass(
            PRIVATE_IGNORE,
            "private zone is effectively ignored",
            "git check-ignore reports the zone ignored",
        ),
        PrivateZoneState::Exposed => with_fix(
            CheckEntry::warn(
                PRIVATE_IGNORE,
                "private zone is effectively ignored",
                "private zone is not effectively ignored — machine-specific knowledge written there would be committed",
            ),
            "turu init",
        ),
    }
}

fn check_private_tracked(facts: &Facts) -> CheckEntry {
    let tracked = tracked_files_under_private_zone(facts);
    if tracked.is_empty() {
        CheckEntry::pass(
            PRIVATE_TRACKED,
            "no tracked files under the private zone",
            "no private knowledge has entered git history",
        )
    } else {
        let list: Vec<String> = tracked
            .iter()
            .map(|p| format!("`{}`", p.display()))
            .collect();
        CheckEntry::fail(
        PRIVATE_TRACKED,
        "no tracked files under the private zone",
        format!(
            "tracked under the private zone: [{}] — git history is forever — prevention only: retraction requires rewriting history",
            list.join(", ")
        ),
        Some(
            "git rm --cached the files and rewrite history (history cannot be retracted by any turu verb)".to_string(),
        ),
    )
    }
}

fn check_private_leaks(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Advisory leak-shape lint on public repo-local knowledge files.
    let leaks = leak_warnings(facts, resolved);
    if leaks.is_empty() {
        CheckEntry::pass(
            PRIVATE_LEAKS,
            "public knowledge files carry no leak-shaped content",
            "no machine paths, hostname assignments, or token-shaped strings",
        )
    } else {
        with_fix(
            CheckEntry::warn(
                PRIVATE_LEAKS,
                "public knowledge files carry no leak-shaped content",
                leaks.join("; "),
            ),
            "move machine-specific knowledge to the --private scope (never pushed)",
        )
    }
}

pub(super) fn with_fix(mut entry: CheckEntry, fix: &str) -> CheckEntry {
    entry.fix = Some(fix.to_string());
    entry
}
