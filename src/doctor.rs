//! Deep workspace diagnostics, following the genesis doctor conventions.
//!
//! Checks are assembled directly as `CheckEntry`s (config- and git-aware,
//! so the `DoctorCheck` trait's `repo_root`-only signature doesn't fit).
//! The private-zone trio (effective ignore, tracked files, leak-shape
//! lint) is the hard guarantee behind the gitignore backstop: doctor
//! detects what `init`/`sync` self-heal and what no verb can retract.

use std::path::Path;

use genesis::doctor::{CheckEntry, DoctorReport};

use crate::config::Resolved;
use crate::skill_pack;
use crate::workspace::{
    Facts, PrivateZoneState, Scope, agents_file, legacy_variants, private_zone_state, resolve,
    scope_files_for, scope_files_for_scoped, tracked_files_under_private_zone, turu_injector,
};

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
const USAGE_STALENESS: &str = "turu.usage-staleness";
const PRIVATE_IGNORE: &str = "turu.private-ignore";
const PRIVATE_TRACKED: &str = "turu.private-tracked";
const PRIVATE_LEAKS: &str = "turu.private-leaks";

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
    for path in scope_files_for(facts, resolved) {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let unranked = crate::entry::parse_file(&raw)
                .into_iter()
                .filter_map(|i| match i {
                    crate::entry::Item::Line(l) if !l.trim().is_empty() => Some(l),
                    _ => None,
                })
                .count();
            if unranked > 0 {
                unmanaged.push(format!("{} ({unranked} freeform line(s))", path.display()));
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
    for path in scope_files_for(facts, resolved) {
        if let Some(parent) = path.parent().map(std::path::Path::to_path_buf) {
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

    // Usage staleness (whisper-t6j): per-scope verdicts against the Ro5
    // gates — last_recalled > 90d, or never-recalled with entry age > 30d.
    // Usage lives in machine-local sidecars; absent sidecars mean telemetry
    // hasn't started, which is a pass, not a warning.
    let now = crate::entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref()).ok();
    let mut stale: Vec<String> = Vec::new();
    let mut never_used: Vec<String> = Vec::new();
    for (scope, path) in scope_files_for_scoped(facts, resolved) {
        if !path.exists() {
            continue;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let stats = crate::usage::read_stats(&path);
        for item in crate::entry::parse_file(&raw) {
            let crate::entry::Item::Entry(e) = item else {
                continue;
            };
            if e.superseded_by.is_some() {
                continue; // superseded entries are dead by decision, not by staleness
            }
            let verdict = match (&now, stats.get(&e.id)) {
                (Some(now), stats) => crate::usage::staleness(&e.ts, stats, now),
                (None, _) => crate::usage::Staleness::Fresh,
            };
            match verdict {
                crate::usage::Staleness::Stale => {
                    stale.push(format!("{} ({})", short_id(&e.id), scope_name(scope)));
                }
                crate::usage::Staleness::NeverUsed => {
                    never_used.push(format!("{} ({})", short_id(&e.id), scope_name(scope)));
                }
                crate::usage::Staleness::Fresh => {}
            }
        }
    }
    checks.push(if stale.is_empty() && never_used.is_empty() {
        CheckEntry::pass(
            USAGE_STALENESS,
            "entries show no usage staleness",
            "all entries recently recalled or within the never-used grace window",
        )
    } else {
        let mut parts: Vec<String> = Vec::new();
        if !stale.is_empty() {
            parts.push(format!(
                "stale (last recalled > {}d ago): [{}]",
                crate::usage::STALE_DAYS,
                stale.join(", ")
            ));
        }
        if !never_used.is_empty() {
            parts.push(format!(
                "never recalled (older than {}d grace): [{}]",
                crate::usage::GRACE_DAYS,
                never_used.join(", ")
            ));
        }
        with_fix(
            CheckEntry::warn(
                USAGE_STALENESS,
                "entries show no usage staleness",
                parts.join(" — "),
            ),
            "distill the stale scopes and prune dead entries (recall usage only counts what was actually served)",
        )
    });

    // Private-zone integrity (add-repo-private-scope 3.2): the checkout
    // self-protects and self-reports. Gitignore is the backstop, turu is
    // the guarantee — doctor owns the hard guarantee.
    checks.push(match private_zone_state(facts) {
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
    });

    let tracked = tracked_files_under_private_zone(facts);
    checks.push(if tracked.is_empty() {
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
                "tracked under the private zone: [{}] — git history is forever: prevention only, retraction requires rewriting history",
                list.join(", ")
            ),
            Some(
                "git rm --cached the files and rewrite history (history cannot be retracted by any turu verb)".to_string(),
            ),
        )
    });

    // Advisory leak-shape lint on public repo-local knowledge files.
    let leaks = leak_warnings(facts, resolved);
    checks.push(if leaks.is_empty() {
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

// ---------------------------------------------------------------------------
// Leak-shape lint (add-repo-private-scope 3.2c) — advisory
// ---------------------------------------------------------------------------

/// Machine-path prefixes that betray a specific developer machine.
const LEAK_PATH_PREFIXES: [&str; 4] = ["C:\\Users\\", "/var/home/", "/home/", "/Users/"];

/// Minimum token length for the token-shaped shape.
const LEAK_TOKEN_MIN: usize = 32;

/// Advisory leak-shape lint over one text: flag machine paths, hostname
/// assignments, and token-shaped strings (API keys, PATs, long hex ids).
/// Detection that warns can be loose; this never blocks anything.
/// Findings carry 1-based line numbers relative to the linted text.
pub fn leak_findings(text: &str) -> Vec<String> {
    let mut findings = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line_no = n + 1;
        for prefix in LEAK_PATH_PREFIXES {
            if let Some(at) = machine_path_at(line, prefix) {
                findings.push(format!(
                    "line {line_no}: machine path `{}`",
                    path_at(&line[at..])
                ));
                break;
            }
        }
        if let Some(found) = hostname_assignment(line) {
            findings.push(format!("line {line_no}: hostname assignment `{found}`"));
        }
        for tok in line.split_whitespace() {
            let t =
                tok.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));
            if is_token_shaped(t) {
                let shown: String = t.chars().take(16).collect();
                findings.push(format!(
                    "line {line_no}: token-shaped string `{shown}…` ({} chars)",
                    t.len()
                ));
            }
        }
    }
    findings
}

/// Advisory leak warnings for the doctor envelope: lint the public
/// repo-local knowledge files (repo env.md, branch notes.md). Entry text
/// is linted, never raw file lines — entry markers carry ids/timestamps
/// that would false-positive every well-formed file (the whisper-4xl
/// lesson); freeform lines are user prose and are linted as-is.
pub fn leak_warnings(facts: &Facts, resolved: &Resolved) -> Vec<String> {
    if facts.repo_local_root.is_none() {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    for scope in [Scope::Repo, Scope::Branch] {
        let Ok(target) = resolve(scope, facts, resolved) else {
            continue;
        };
        if crate::workspace::is_in_private_zone(&target.path, facts) || !target.path.exists() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&target.path) else {
            continue;
        };
        let mut linted: Vec<String> = Vec::new();
        for item in crate::entry::parse_file(&raw) {
            match item {
                crate::entry::Item::Entry(e) => linted.extend(leak_findings(&e.text)),
                crate::entry::Item::Line(l) => {
                    if !l.trim().is_empty() {
                        linted.extend(leak_findings(&l));
                    }
                }
            }
        }
        if !linted.is_empty() {
            warnings.push(format!(
                "leak-shaped content in `{}`: {}",
                target.path.display(),
                linted.join(", ")
            ));
        }
    }
    warnings
}

/// Start of a machine path at a word boundary (not inside another word —
/// `/var/home/` contains `/home/` but only as a non-boundary suffix).
fn machine_path_at(line: &str, prefix: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(idx) = line[from..].find(prefix) {
        let at = from + idx;
        let boundary = at == 0
            || !line[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
        if boundary {
            return Some(at);
        }
        from = at + prefix.len().max(1);
    }
    None
}

/// Path string starting at `at`: extends over alphanumerics and `/-_.~`.
fn path_at(rest: &str) -> String {
    rest.chars()
        .take_while(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~' | '\\')
        })
        .collect()
}

/// A `host:`/`hostname=`-shaped assignment: the word `host` (optionally
/// `name`), optional spaces, `:` or `=`, then a value. Word-bounded so
/// `localhost:8080` stays quiet. Returns the assigned value.
fn hostname_assignment(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let mut from = 0;
    while let Some(idx) = lower[from..].find("host") {
        let at = from + idx;
        let boundary = at == 0
            || !lower[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
        if boundary {
            let after = &lower[at + 4..];
            let after = after.strip_prefix("name").unwrap_or(after);
            let value_part = after.trim_start();
            if let Some(v) = value_part
                .strip_prefix(':')
                .or_else(|| value_part.strip_prefix('='))
            {
                let v = v.trim();
                if !v.is_empty() {
                    let end = v.find(char::is_whitespace).unwrap_or(v.len());
                    return Some(v[..end].to_string());
                }
            }
        }
        from = at + 4;
    }
    None
}

/// Token-shaped: long (≥32) run of alphanumerics/`_-` containing both a
/// digit and a letter — API keys, PATs, long hex ids. Hex words without
/// digits (`deadbeef…`) stay quiet.
fn is_token_shaped(tok: &str) -> bool {
    tok.len() >= LEAK_TOKEN_MIN
        && tok
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && tok.chars().any(|c| c.is_ascii_digit())
        && tok.chars().any(|c| c.is_ascii_alphabetic())
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(12)]
}

fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Repo => "repo",
        Scope::Branch => "branch",
        Scope::Worktree => "worktree",
        Scope::Group => "group",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leak_lint_flags_machine_paths() {
        for p in [
            "/home/sasha/infra",
            "/Users/sasha/infra",
            "/var/home/sasha/infra",
            "C:\\Users\\sasha",
        ] {
            let findings = leak_findings(&format!("deploy runs from {p}"));
            assert!(
                findings.iter().any(|m| m.contains("machine path")),
                "{p}: {findings:?}"
            );
        }
    }

    #[test]
    fn leak_lint_flags_hostname_assignments() {
        let findings = leak_findings("built on host: sasha-fedora and hostname=brix");
        assert!(
            findings.iter().any(|m| m.contains("hostname")),
            "{findings:?}"
        );
    }

    #[test]
    fn leak_lint_flags_token_shaped_strings() {
        for tok in [
            "ghp_0123456789abcdef0123456789abcdef0123",
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
        ] {
            let findings = leak_findings(&format!("token {tok}"));
            assert!(
                findings.iter().any(|m| m.contains("token-shaped")),
                "{tok}: {findings:?}"
            );
        }
    }

    #[test]
    fn leak_lint_stays_quiet_on_ordinary_lessons() {
        assert!(
            leak_findings("prefer O_APPEND fast path; full rewrite only for --supersedes")
                .is_empty()
        );
        assert!(leak_findings("entry ids are sha256 over scope-key+ts+text").is_empty());
        assert!(leak_findings("ts 2026-09-28T21:48:17Z").is_empty());
        // Hex words without digits never trip the token shape.
        assert!(
            leak_findings("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef is not a token").is_empty()
        );
    }

    #[test]
    fn leak_lint_reports_line_numbers() {
        let findings = leak_findings("ok line\n/home/sasha/infra here");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("line 2"), "{findings:?}");
        assert!(findings[0].contains("/home/sasha/infra"), "{findings:?}");
    }
}
