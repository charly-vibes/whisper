//! Group/shadowed-global and managed-file doctor checks.

use std::path::Path;

use genesis::doctor::CheckEntry;

use super::checks::with_fix;
use super::{GROUP_ROOT, MANAGED_BLOCK, MANAGED_SKILLS, SHADOWED_GLOBAL};
use crate::config::Resolved;
use crate::skill_pack;
use crate::workspace::{agents_file, turu_injector};

/// The group-root check (only when a group is active).
pub(super) fn check_group_root(resolved: &Resolved) -> Option<CheckEntry> {
    let (name, root) = resolved.group.as_ref()?;
    Some(if root.exists() {
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
    })
}
/// The shadowed-global check: a repo-private workspace_root relocates
/// `rules.md`, so the repo can silently lose sight of the shared global
/// rules. Surface the shadowing and any divergence.
pub(super) fn check_shadowed_global(resolved: &Resolved) -> Option<CheckEntry> {
    let real_root = resolved.shadowed_global_root.as_ref()?;
    let real_rules = real_root.join("rules.md");
    let shadowed_rules = resolved.workspace_root.join("rules.md");
    Some(if !real_rules.exists() {
        CheckEntry::pass(
            SHADOWED_GLOBAL,
            "repo-private root does not hide a global rules file",
            format!(
                "repo-private root shadows the global scope, but `{}` is absent — nothing hidden",
                real_rules.display()
            ),
        )
    } else if !shadowed_rules.exists() {
        with_fix(
            CheckEntry::warn(
                SHADOWED_GLOBAL,
                "repo-private root does not hide a global rules file",
                format!(
                    "repo-private root `{}` hides the global rules file at `{}` — the repo no longer sees it",
                    resolved.workspace_root.display(),
                    real_rules.display()
                ),
            ),
            "copy the global rules.md into the private root, or remove the repo-private workspace_root override",
        )
    } else {
        shadow_rules_verdict(&real_rules, &shadowed_rules)
    })
}

/// When both rules.md files exist: pass when in sync, warn on divergence.
fn shadow_rules_verdict(real_rules: &Path, shadowed_rules: &Path) -> CheckEntry {
    let real = std::fs::read_to_string(real_rules).unwrap_or_default();
    let shadowed = std::fs::read_to_string(shadowed_rules).unwrap_or_default();
    if real == shadowed {
        CheckEntry::pass(
            SHADOWED_GLOBAL,
            "repo-private root does not hide a global rules file",
            format!(
                "repo-private root shadows the global scope, but rules.md is in sync with `{}`",
                real_rules.display()
            ),
        )
    } else {
        with_fix(
            CheckEntry::warn(
                SHADOWED_GLOBAL,
                "repo-private root does not hide a global rules file",
                format!(
                    "shadowed rules.md at `{}` has diverged from the global rules.md at `{}`",
                    shadowed_rules.display(),
                    real_rules.display()
                ),
            ),
            "reconcile the two rules.md files (copy the global one over, or port the local changes back)",
        )
    }
}
/// The managed block check: `AGENTS.md` carries `<!-- TURU:START -->`.
pub(super) fn check_managed_block(repo_root: &Path) -> CheckEntry {
    // Managed block in the agent-facing file.
    let agents = agents_file(repo_root);
    let has_block = agents.exists()
        && turu_injector().registry().get("turu").is_some_and(|_| {
            std::fs::read_to_string(&agents)
                .map(|raw| raw.contains("<!-- TURU:START -->"))
                .unwrap_or(false)
        });
    if has_block {
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
    }
}

/// The managed skill pack staleness check (optional pack: missing = pass).
pub(super) fn check_managed_skills(repo_root: &Path) -> CheckEntry {
    let pack_dir = crate::workspace::repo_root(repo_root)
        .join(".turu")
        .join("skills")
        .join("whisper");
    if !pack_dir.exists() {
        return CheckEntry::pass(
            MANAGED_SKILLS,
            "whisper skill pack is current when installed",
            "pack not installed (optional) — `turu skill install`",
        );
    }
    match skill_pack::generate_pack("whisper") {
        Ok(generated) => {
            let expected = skill_pack::pack_content_hash(&generated);
            Ok(pack_verdict(&pack_dir, &expected))
        }
        Err(msg) => Err(msg),
    }
    .unwrap_or_else(|msg| {
        CheckEntry::fail(
            MANAGED_SKILLS,
            "whisper skill pack is current when installed",
            msg,
            None,
        )
    })
}

/// Pack verdict when generation succeeded: current vs stale.
fn pack_verdict(pack_dir: &Path, expected: &str) -> CheckEntry {
    match skill_pack::disk_content_hash(pack_dir) {
        Ok(disk) if disk == expected => CheckEntry::pass(
            MANAGED_SKILLS,
            "whisper skill pack is current when installed",
            format!("`{}` is current", pack_dir.display()),
        ),
        _ => with_fix(
            CheckEntry::warn(
                MANAGED_SKILLS,
                "whisper skill pack is current when installed",
                format!("`{}` is stale", pack_dir.display()),
            ),
            "turu skill install",
        ),
    }
}
