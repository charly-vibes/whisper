//! Usage telemetry: append-only, machine-local records of what recall
//! actually served, keyed by entry id (whisper-t6j).
//!
//! One JSONL record per served recall: `{"id":"<sha2>","ts":"<RFC3339>"}`.
//! The sidecar (`<scope-file-stem>.usage.jsonl`) lives next to its scope
//! file, so it follows `consolidate` moves and is excluded from bundles by
//! construction (bundles carry scope files, never siblings). Records are
//! append-only — corrupt or foreign lines are skipped on read and left
//! untouched on write. Superseding an entry orphans its old-id records;
//! orphans are ignored, never a doctor warning.

use std::collections::BTreeMap;
use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::{Result, WhisperError, entry};

/// One usage record: entry id + the second-precision UTC ts of the recall
/// that served it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub ts: String,
}

/// Aggregated per-entry usage, as surfaced to doctor and distill.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    pub use_count: u64,
    pub last_recalled: Option<String>,
}

/// Per-entry staleness verdict against the Ro5 gates: warn when
/// `last_recalled` is older than `STALE_DAYS`, or when an entry older than
/// `GRACE_DAYS` has never been recalled. Entries in grace are quiet.
pub const STALE_DAYS: i64 = 90;
pub const GRACE_DAYS: i64 = 30;

pub fn staleness(entry_ts: &str, stats: Option<&Stats>, now: &str) -> Staleness {
    let parse = |ts: &str| {
        time::OffsetDateTime::parse(ts, &time::format_description::well_known::Rfc3339).ok()
    };
    let (Some(entry_dt), Some(now_dt)) = (parse(entry_ts), parse(now)) else {
        // Unparseable timestamps must never crash doctor — treat as fresh.
        return Staleness::Fresh;
    };
    let age_days = (now_dt - entry_dt).whole_days();
    match stats {
        Some(s) if s.use_count > 0 => {
            let last = s
                .last_recalled
                .as_deref()
                .and_then(parse)
                .unwrap_or(entry_dt);
            if (now_dt - last).whole_days() > STALE_DAYS {
                Staleness::Stale
            } else {
                Staleness::Fresh
            }
        }
        _ if age_days > GRACE_DAYS => Staleness::NeverUsed,
        _ => Staleness::Fresh,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    Fresh,
    Stale,
    NeverUsed,
}

/// Recency map for one scope file: entry id → stats, for distill envelopes.
pub fn recency_for(items: &[entry::Item], scope_file: &std::path::Path) -> BTreeMap<String, Stats> {
    let all = read_stats(scope_file);
    entry::entry_ids(items)
        .into_iter()
        .filter_map(|id| all.get(id).cloned().map(|s| (id.to_string(), s)))
        .collect()
}

/// Sidecar path for a scope file: same stem, `usage.jsonl` extension
/// (`notes.md` → `notes.usage.jsonl`).
pub fn sidecar_path(scope_file: &std::path::Path) -> std::path::PathBuf {
    scope_file.with_extension("usage.jsonl")
}

/// Append usage records to a scope's sidecar in one O_APPEND write. Creates
/// the sidecar (and parents) only when there is something to record.
pub fn append(scope_file: &std::path::Path, ids: &[String], ts: &str) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let sidecar = sidecar_path(scope_file);
    if let Some(parent) = sidecar.parent() {
        std::fs::create_dir_all(parent).map_err(WhisperError::from)?;
    }
    let mut buf = String::new();
    for id in ids {
        let rec = Record {
            id: id.clone(),
            ts: ts.to_string(),
        };
        buf.push_str(
            &serde_json::to_string(&rec)
                .map_err(|e| WhisperError::new(format!("serializing usage record: {e}")))?,
        );
        buf.push('\n');
    }
    // O_APPEND: each write lands at the end; concurrent turu runs never
    // overwrite each other's records.
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&sidecar)
        .map_err(WhisperError::from)?;
    f.write_all(buf.as_bytes()).map_err(WhisperError::from)?;
    Ok(ids.len())
}

/// Read a sidecar into per-entry stats. Tolerant: corrupt or non-JSON lines
/// are skipped (append-only means they may be foreign debris); lines without
/// an `id` are ignored.
pub fn read_stats(scope_file: &std::path::Path) -> BTreeMap<String, Stats> {
    let mut stats: BTreeMap<String, Stats> = BTreeMap::new();
    let Ok(raw) = std::fs::read_to_string(sidecar_path(scope_file)) else {
        return stats;
    };
    for line in raw.lines() {
        let Ok(rec) = serde_json::from_str::<Record>(line) else {
            continue;
        };
        if rec.id.is_empty() {
            continue;
        }
        let e = stats.entry(rec.id).or_default();
        e.use_count += 1;
        if e.last_recalled
            .as_deref()
            .is_none_or(|last| rec.ts.as_str() > last)
        {
            e.last_recalled = Some(rec.ts);
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn scope_file(dir: &std::path::Path) -> std::path::PathBuf {
        dir.join("notes.md")
    }

    #[test]
    fn sidecar_sits_next_to_the_scope_file() {
        let p = std::path::PathBuf::from("/ws/repos/x/branches/main/notes.md");
        assert_eq!(
            sidecar_path(&p),
            std::path::PathBuf::from("/ws/repos/x/branches/main/notes.usage.jsonl")
        );
    }

    #[test]
    fn append_is_one_o_append_write_and_creates_parents() {
        let tmp = TempDir::new().unwrap();
        let f = scope_file(tmp.path()); // parents not created yet
        let n = append(
            &f,
            &["a".repeat(64), "b".repeat(64)],
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(n, 2);
        let raw = std::fs::read_to_string(sidecar_path(&f)).unwrap();
        assert_eq!(raw.lines().count(), 2);
        assert!(raw.lines().all(|l| l.starts_with('{') && l.ends_with('}')));
    }

    #[test]
    fn append_nothing_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let f = scope_file(tmp.path());
        assert_eq!(append(&f, &[], "2026-01-01T00:00:00Z").unwrap(), 0);
        assert!(!sidecar_path(&f).exists());
    }

    #[test]
    fn read_stats_counts_and_keeps_latest_ts() {
        let tmp = TempDir::new().unwrap();
        let f = scope_file(tmp.path());
        let id = "a".repeat(64);
        append(&f, std::slice::from_ref(&id), "2026-01-02T00:00:00Z").unwrap();
        append(&f, std::slice::from_ref(&id), "2026-01-05T00:00:00Z").unwrap();
        let stats = read_stats(&f);
        let s = stats.get(id.as_str()).unwrap();
        assert_eq!(s.use_count, 2);
        assert_eq!(s.last_recalled.as_deref(), Some("2026-01-05T00:00:00Z"));
    }

    #[test]
    fn read_stats_skips_corrupt_lines_but_keeps_the_rest() {
        let tmp = TempDir::new().unwrap();
        let f = scope_file(tmp.path());
        std::fs::write(
            sidecar_path(&f),
            "garbage\n{\"id\":\"x\",\"ts\":\"2026-01-01T00:00:00Z\"}\n\n",
        )
        .unwrap();
        let stats = read_stats(&f);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats["x"].use_count, 1);
    }

    #[test]
    fn read_stats_missing_sidecar_is_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read_stats(&scope_file(tmp.path())).is_empty());
    }

    #[test]
    fn staleness_gates_follow_the_ro5_verdict() {
        let now = "2026-05-02T00:00:00Z";
        let id = "a".repeat(64);
        // Never used, within the 30d grace → fresh.
        assert_eq!(
            staleness("2026-04-20T00:00:00Z", None, now),
            Staleness::Fresh
        );
        // Never used, beyond grace → NeverUsed.
        assert_eq!(
            staleness("2026-01-01T00:00:00Z", None, now),
            Staleness::NeverUsed
        );
        // Used recently → fresh.
        let hot = Stats {
            use_count: 3,
            last_recalled: Some("2026-04-30T00:00:00Z".into()),
        };
        assert_eq!(
            staleness("2026-01-01T00:00:00Z", Some(&hot), now),
            Staleness::Fresh
        );
        // Used but last recall > 90d → Stale.
        let stale = Stats {
            use_count: 1,
            last_recalled: Some("2026-01-02T00:00:00Z".into()),
        };
        assert_eq!(
            staleness("2026-01-01T00:00:00Z", Some(&stale), now),
            Staleness::Stale
        );
        // Unparseable timestamps degrade to fresh, never crash doctor.
        assert_eq!(staleness("banana", Some(&stale), now), Staleness::Fresh);
        assert_eq!(
            staleness("2026-01-01T00:00:00Z", Some(&stale), "nonsense"),
            Staleness::Fresh
        );
        let _ = id;
    }

    #[test]
    fn recency_map_covers_only_entries_in_the_file() {
        let tmp = TempDir::new().unwrap();
        let f = scope_file(tmp.path());
        let e = entry::Entry {
            ts: "2026-01-01T00:00:00Z".into(),
            id: entry::entry_id("s", "2026-01-01T00:00:00Z", "known"),
            topic: None,
            text: "known".into(),
            supersedes: None,
            superseded_by: None,
        };
        append(&f, std::slice::from_ref(&e.id), "2026-01-02T00:00:00Z").unwrap();
        let items = vec![entry::Item::Entry(e)];
        let recency = recency_for(&items, &f);
        assert_eq!(recency.len(), 1);
        assert_eq!(recency.values().next().unwrap().use_count, 1);
    }
}
