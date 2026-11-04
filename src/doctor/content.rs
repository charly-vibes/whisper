//! Knowledge-content doctor checks: entry format, pending distill
//! revisions, and per-scope usage staleness (whisper-t6j).

use genesis::doctor::CheckEntry;

use super::checks::with_fix;
use super::leaks::{scope_name, short_id};
use super::{DISTILL_PENDING, ENTRY_FORMAT, USAGE_STALENESS};
use crate::config::Resolved;
use crate::workspace::{Facts, Scope, scope_files_for, scope_files_for_scoped};

pub(super) fn content_checks(facts: &Facts, resolved: &Resolved) -> Vec<CheckEntry> {
    vec![
        check_entry_format(facts, resolved),
        check_distill_pending(facts, resolved),
        check_usage_staleness(facts, resolved),
    ]
}

/// // Global rules file.
// they carry no entry ids, so recall/bundle treat them as opaque text.
fn check_entry_format(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Unmanaged (freeform) lines in knowledge files: informational —
    // they carry no entry ids, so recall/bundle treat them as opaque text.
    let unmanaged = unmanaged_files(facts, resolved);
    if unmanaged.is_empty() {
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
    }
}

/// Scope files carrying freeform (unmanaged) lines: path + line count.
fn unmanaged_files(facts: &Facts, resolved: &Resolved) -> Vec<String> {
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
    unmanaged
}

/// Per-scope staleness scan (whisper-t6j): Ro5 gates — last_recalled
/// > 90d, or never-recalled with entry age > 30d. Absent sidecars mean
/// telemetry hasn't started: entries stay fresh, not stale.
/// Returns (stale ids, never-used ids) as display strings.
fn staleness_lists(facts: &Facts, resolved: &Resolved) -> (Vec<String>, Vec<String>) {
    let now = crate::entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref()).ok();
    let mut labeled: Vec<(crate::usage::Staleness, String)> = Vec::new();
    for (scope, path) in scope_files_for_scoped(facts, resolved) {
        labeled.extend(scope_staleness(scope, &path, &now));
    }
    let is_stale =
        |(v, _): &(crate::usage::Staleness, String)| matches!(v, crate::usage::Staleness::Stale);
    let stale = labeled
        .iter()
        .filter(|(v, _)| matches!(v, crate::usage::Staleness::Stale))
        .map(|(_, l)| l.clone())
        .collect();
    let never_used = labeled
        .iter()
        .filter(|(v, _)| matches!(v, crate::usage::Staleness::NeverUsed))
        .map(|(_, l)| l.clone())
        .collect();
    let _ = is_stale;
    (stale, never_used)
}

/// Staleness labels for one scope file's live entries (missing file → none).
fn scope_staleness(
    scope: Scope,
    path: &std::path::Path,
    now: &Option<String>,
) -> Vec<(crate::usage::Staleness, String)> {
    if !path.exists() {
        return Vec::new();
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let stats = crate::usage::read_stats(path);
    crate::entry::parse_file(&raw)
        .into_iter()
        .filter_map(|item| match item {
            crate::entry::Item::Entry(e) if e.superseded_by.is_none() => Some(e),
            _ => None, // superseded entries are dead by decision, not by staleness
        })
        .map(|e| {
            let verdict = match (now, stats.get(&e.id)) {
                (Some(now), stats) => crate::usage::staleness(&e.ts, stats, now),
                (None, _) => crate::usage::Staleness::Fresh,
            };
            let label = format!("{} ({})", short_id(&e.id), scope_name(scope));
            (verdict, label)
        })
        .collect()
}

/// Pending distill revisions: begun but not committed (informational).
fn check_distill_pending(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Pending distill revisions: begun but not committed (informational).
    let mut pending: Vec<String> = Vec::new();
    for path in scope_files_for(facts, resolved) {
        if let Some(parent) = path.parent().map(std::path::Path::to_path_buf) {
            for id in crate::distill::pending_revisions(&parent) {
                pending.push(format!("{} (in {})", id, parent.display()));
            }
        }
    }
    if pending.is_empty() {
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
    }
}

/// // Usage staleness (whisper-t6j): per-scope verdicts against the Ro5
// gates — last_recalled > 90d, or never-recalled with entry age > 30d.
// Usage lives in machine-local sidecars; absent sidecars mean telemetry
// hasn't started, which is a pass, not a warning.
fn check_usage_staleness(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    // Usage staleness (whisper-t6j): per-scope verdicts against the Ro5
    // gates — last_recalled > 90d, or never-recalled with entry age > 30d.
    // Usage lives in machine-local sidecars; absent sidecars mean telemetry
    // hasn't started, which is a pass, not a warning.
    let (stale, never_used) = staleness_lists(facts, resolved);
    if stale.is_empty() && never_used.is_empty() {
        return CheckEntry::pass(
            USAGE_STALENESS,
            "entries show no usage staleness",
            "all entries recently recalled or within the never-used grace window",
        );
    }
    staleness_warning(&stale, &never_used)
}

/// The warning entry for stale / never-used entries.
fn staleness_warning(stale: &[String], never_used: &[String]) -> CheckEntry {
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
}
