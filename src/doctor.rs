//! Deep workspace diagnostics, following the genesis doctor conventions.
//!
//! Checks are assembled directly as `CheckEntry`s (config- and git-aware,
//! so the `DoctorCheck` trait's `repo_root`-only signature doesn't fit).

use std::path::Path;

use genesis::doctor::{CheckEntry, DoctorReport};

use crate::config::Resolved;
use crate::skill_pack;
use crate::workspace::{Facts, Scope, agents_file, legacy_variants, resolve, turu_injector};

const REPO_SLOT: &str = "turu.repo-slot";
const RULES: &str = "turu.rules-md";
const BRANCH_SLOT: &str = "turu.branch-slot";
const LEGACY_KEYS: &str = "turu.legacy-keys";
const GROUP_ROOT: &str = "turu.group-root";
const SHADOWED_GLOBAL: &str = "turu.shadowed-global";
const MANAGED_BLOCK: &str = "turu.managed-block";
const MANAGED_SKILLS: &str = "turu.managed-skills";
const ENTRY_FORMAT: &str = "turu.entry-format";
const DISTILL_PENDING: &str = "turu.distill-pending";

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
            "turu consolidate",
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

    // Repo-private workspace_root shadowing the global scope: the override
    // also relocates `rules.md`, so the repo can silently lose sight of the
    // shared global rules. Surface the shadowing and any divergence.
    if let Some(real_root) = &resolved.shadowed_global_root {
        let real_rules = real_root.join("rules.md");
        let shadowed_rules = resolved.workspace_root.join("rules.md");
        checks.push(if !real_rules.exists() {
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
            let real = std::fs::read_to_string(&real_rules).unwrap_or_default();
            let shadowed = std::fs::read_to_string(&shadowed_rules).unwrap_or_default();
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
        });
    }

    // Unmanaged (freeform) lines in knowledge files: informational —
    // they carry no entry ids, so recall/bundle treat them as opaque text.
    let mut unmanaged: Vec<String> = Vec::new();
    for scope in [Scope::Global, Scope::Repo, Scope::Branch, Scope::Worktree] {
        let raw = resolve(scope, facts, resolved)
            .ok()
            .filter(|t| t.path.exists())
            .and_then(|t| std::fs::read_to_string(&t.path).ok());
        if let Some(raw) = raw {
            let unranked = crate::entry::parse_file(&raw)
                .into_iter()
                .filter_map(|i| match i {
                    crate::entry::Item::Line(l) if !l.trim().is_empty() => Some(l),
                    _ => None,
                })
                .count();
            if unranked > 0 {
                unmanaged.push(format!(
                    "{} ({unranked} freeform line(s))",
                    resolve(scope, facts, resolved).unwrap().path.display()
                ));
            }
        }
    }
    checks.push(if unmanaged.is_empty() {
        CheckEntry::pass(
            ENTRY_FORMAT,
            "knowledge files carry only managed entry lines",
            "no unmanaged content in scope files",
        )
    } else {
        CheckEntry::warn(
            ENTRY_FORMAT,
            "knowledge files carry only managed entry lines",
            format!(
                "freeform (unmanaged) content found in: {}",
                unmanaged.join(", ")
            ),
        )
    });

    // Pending distill revisions: begun but not committed (informational).
    let mut pending: Vec<String> = Vec::new();
    for scope in [Scope::Global, Scope::Repo, Scope::Branch, Scope::Worktree] {
        let parent = resolve(scope, facts, resolved)
            .ok()
            .and_then(|t| t.path.parent().map(std::path::Path::to_path_buf));
        if let Some(parent) = parent {
            for id in crate::distill::pending_revisions(&parent) {
                pending.push(format!("{} (in {})", id, parent.display()));
            }
        }
    }
    checks.push(if pending.is_empty() {
        CheckEntry::pass(
            DISTILL_PENDING,
            "no distill revisions left uncommitted",
            "no pending revisions",
        )
    } else {
        CheckEntry::warn(
            DISTILL_PENDING,
            "no distill revisions left uncommitted",
            format!(
                "pending: {} — commit with `turu distill <scope> --commit --revision <id>`",
                pending.join(", ")
            ),
        )
    });

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

    // Managed skill pack staleness (optional pack: missing = pass).
    let pack_dir = crate::workspace::repo_root(repo_root)
        .join(".turu")
        .join("skills")
        .join("whisper");
    let pack_check = if !pack_dir.exists() {
        CheckEntry::pass(
            MANAGED_SKILLS,
            "whisper skill pack is current when installed",
            "pack not installed (optional) — `turu skill install`",
        )
    } else {
        match skill_pack::generate_pack("whisper") {
            Ok(generated) => {
                let expected = skill_pack::pack_content_hash(&generated);
                match skill_pack::disk_content_hash(&pack_dir) {
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
            Err(msg) => CheckEntry::fail(
                MANAGED_SKILLS,
                "whisper skill pack is current when installed",
                msg,
                None,
            ),
        }
    };
    checks.push(pack_check);

    DoctorReport::new("turu", checks)
}

fn with_fix(mut entry: CheckEntry, fix: &str) -> CheckEntry {
    entry.fix = Some(fix.to_string());
    entry
}
