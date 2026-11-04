//! The distill contract: turu owns the mechanics (snapshot, revision,
//! conflict detection, install, audit); the calling agent owns the semantic
//! rewrite. Revisions are immutable and never pruned.

use serde::Serialize;
use serde_json::json;
use std::fs;

use crate::entry;
use crate::workspace::{self, Target};
use crate::{Result, WhisperError};

/// Snapshot + working paths for one revision.
#[derive(Debug, Serialize)]
pub struct Revision {
    pub id: String,
    pub snapshot_path: std::path::PathBuf,
    pub working_path: std::path::PathBuf,
    pub target_path: std::path::PathBuf,
    pub sha256: String,
    /// Per-entry usage stats (entry id → use_count/last_recalled) so the
    /// rewrite pass can prune dead entries. Omitted when the sidecar is
    /// absent (no telemetry yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recency: Option<std::collections::BTreeMap<String, crate::usage::Stats>>,
}

#[derive(Debug, Serialize)]
pub struct CommitReport {
    pub revision: String,
    pub replaced: std::path::PathBuf,
    pub snapshot_path: std::path::PathBuf,
    /// True when the revision was already committed with identical content.
    pub noop: bool,
}

/// `turu distill <scope> --begin`: snapshot the scope's current state into
/// an immutable revision dir and return the working paths.
pub fn begin(target: &Target, ts: &str) -> Result<Revision> {
    if !target.path.exists() {
        return Err(WhisperError::new(format!(
            "scope file does not exist: {}",
            target.path.display()
        ))
        .with_suggestion("turu init to create the layout (an empty file needs no distill)"));
    }
    let raw = std::fs::read_to_string(&target.path).map_err(WhisperError::from)?;
    let sha = entry::sha256_hex(&raw);
    let id = format!("{}-{}", ts.replace([':', '-'], ""), &sha[..8]);
    let rev_dir = revision_dir(target, &id)?;
    let snapshot_path = rev_dir.join("snapshot.md");
    let working_path = rev_dir.join("working.md");
    fs::write(&snapshot_path, &raw).map_err(WhisperError::from)?;
    write_revision_meta(&rev_dir, &id, target, &sha, &snapshot_path, &working_path)?;
    // Recency signal for the rewrite pass (empty map → omitted).
    let items = entry::parse_file(&raw);
    let recency = crate::usage::recency_for(&items, &target.path);
    let recency = (!recency.is_empty()).then_some(recency);
    Ok(Revision {
        id,
        snapshot_path,
        working_path,
        target_path: target.path.clone(),
        sha256: sha,
        recency,
    })
}

/// Create (or refuse to re-create) the immutable revision directory.
fn revision_dir(target: &Target, id: &str) -> Result<std::path::PathBuf> {
    let rev_dir = target.path.parent().unwrap().join("revisions").join(id);
    if rev_dir.exists() {
        return Err(
            WhisperError::new(format!("revision already exists: {}", rev_dir.display()))
                .with_suggestion(
                    "a distill cycle already began at this second — pass TURU_NOW or retry",
                ),
        );
    }
    fs::create_dir_all(&rev_dir).map_err(WhisperError::from)?;
    Ok(rev_dir)
}

/// Write meta.json: the agent rewrites `working.md`; commit verifies the
/// live file did not drift since this snapshot.
fn write_revision_meta(
    rev_dir: &std::path::Path,
    id: &str,
    target: &Target,
    sha: &str,
    snapshot_path: &std::path::Path,
    working_path: &std::path::Path,
) -> Result<()> {
    fs::write(
        rev_dir.join("meta.json"),
        json!({
            "revision": id,
            "target_path": target.path,
            "target_sha256": sha,
            "snapshot_path": snapshot_path,
            "working_path": working_path,
        })
        .to_string(),
    )
    .map_err(WhisperError::from)
}

fn load_meta(
    rev_dir: &std::path::Path,
) -> Result<(String, String, std::path::PathBuf, std::path::PathBuf)> {
    let raw = std::fs::read_to_string(rev_dir.join("meta.json")).map_err(|_| {
        WhisperError::new(format!(
            "missing revision metadata in {}",
            rev_dir.display()
        ))
        .with_suggestion("re-run `turu distill <scope> --begin`")
    })?;
    let meta: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| WhisperError::new(format!("corrupt revision metadata: {e}")))?;
    Ok((
        meta["revision"].as_str().unwrap_or_default().to_string(),
        meta["target_sha256"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        meta["working_path"]
            .as_str()
            .map(std::path::PathBuf::from)
            .unwrap_or_default(),
        meta["snapshot_path"]
            .as_str()
            .map(std::path::PathBuf::from)
            .unwrap_or_default(),
    ))
}

/// `turu distill <scope> --commit --revision <id>`: install the distilled
/// output only if the live files are unchanged since `--begin`.
pub fn commit(target: &Target, revision: &str) -> Result<CommitReport> {
    let rev_dir = revision_dir_for_commit(target, revision)?;
    let (id, begin_sha, working_path, snapshot_path) = load_meta(&rev_dir)?;
    let committed_marker = rev_dir.join(".committed");
    let live = fs::read_to_string(&target.path).map_err(WhisperError::from)?;

    if let Some(report) = replay_report(
        &committed_marker,
        &live,
        target,
        id.clone(),
        snapshot_path.clone(),
    )? {
        return Ok(report);
    }
    check_drift(&live, &begin_sha)?;
    install(target, &working_path, &committed_marker)?;

    Ok(CommitReport {
        revision: id,
        replaced: target.path.clone(),
        snapshot_path,
        noop: false,
    })
}

/// Resolve + validate the revision directory for a commit.
fn revision_dir_for_commit(target: &Target, revision: &str) -> Result<std::path::PathBuf> {
    if !valid_revision_id(revision) {
        return Err(WhisperError::new(format!(
            "invalid revision id: {revision}"
        ))
        .with_suggestion(
            "revision ids look like 20260105T000000Z-1a2b3c4d — take them from the --begin envelope",
        ));
    }
    let rev_dir = target
        .path
        .parent()
        .unwrap()
        .join("revisions")
        .join(revision);
    if !rev_dir.exists() {
        return Err(
            WhisperError::new(format!("unknown revision: {revision}")).with_suggestion(
                "turu distill <scope> --begin first; revision ids are listed in its envelope",
            ),
        );
    }
    Ok(rev_dir)
}

/// Replay: a committed revision with an undrifted target is a no-op; a
/// committed revision with drift is refused (immutability).
#[allow(clippy::type_complexity)]
fn replay_report(
    committed_marker: &std::path::Path,
    live: &str,
    target: &Target,
    id: String,
    snapshot_path: std::path::PathBuf,
) -> Result<Option<CommitReport>> {
    if !committed_marker.exists() {
        return Ok(None);
    }
    let committed_sha = fs::read_to_string(committed_marker)
        .map_err(WhisperError::from)?
        .trim()
        .to_string();
    if entry::sha256_hex(live) == committed_sha {
        return Ok(Some(CommitReport {
            revision: id,
            replaced: target.path.clone(),
            snapshot_path,
            noop: true,
        }));
    }
    Err(
        WhisperError::new("revision is already committed and the target drifted since")
            .with_suggestion("run a new `turu distill <scope> --begin` cycle"),
    )
}

/// Conflict detection: live file must be byte-identical to --begin.
fn check_drift(live: &str, begin_sha: &str) -> Result<()> {
    let live_sha = entry::sha256_hex(live);
    if live_sha != begin_sha {
        return Err(WhisperError::new(format!(
            "scope file changed since --begin (concurrent write?) — expected sha {}, live {}",
            &begin_sha[..12],
            &live_sha[..12]
        ))
        .with_suggestion("re-run `turu distill <scope> --begin` and redo the rewrite"));
    }
    Ok(())
}

/// Install the distilled working file: mark it committed, then atomically
/// rename it over the live scope file (same filesystem).
fn install(
    target: &Target,
    working_path: &std::path::Path,
    committed_marker: &std::path::Path,
) -> Result<()> {
    let distilled = fs::read_to_string(working_path).map_err(|_| {
        WhisperError::new(format!(
            "working file not written: {}",
            working_path.display()
        ))
        .with_suggestion(format!(
            "write the distilled content to {} then re-run --commit",
            working_path.display()
        ))
    })?;
    let distilled_sha = entry::sha256_hex(&distilled);
    fs::write(committed_marker, &distilled_sha).map_err(WhisperError::from)?;
    // Atomic install: same filesystem, rename replaces the live file.
    fs::rename(working_path, &target.path).map_err(WhisperError::from)
}

/// Strict revision-id grammar: `<compact-timestamp>-<8 hex>` — also closes
/// path traversal via a crafted --revision.
fn valid_revision_id(rev: &str) -> bool {
    let Some((ts, hash)) = rev.split_once('-') else {
        return false;
    };
    !ts.is_empty()
        && ts.chars().all(|c| c.is_ascii_alphanumeric())
        && hash.len() == 8
        && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Doctor helper: revisions begun but not committed (informational).
pub fn pending_revisions(target_dir: &std::path::Path) -> Vec<String> {
    let rev_root = target_dir.join("revisions");
    let Ok(entries) = fs::read_dir(&rev_root) else {
        return Vec::new();
    };
    let mut pending = Vec::new();
    for e in entries.flatten() {
        let dir = e.path();
        if dir.is_dir() && dir.join("meta.json").exists() && !dir.join(".committed").exists() {
            pending.push(
                dir.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
        }
    }
    pending.sort();
    pending
}

/// Scope-keyed convenience wrappers used by the CLI.
pub fn begin_for_scope(
    scope: workspace::Scope,
    facts: &workspace::Facts,
    resolved: &crate::config::Resolved,
    ts: &str,
) -> Result<Revision> {
    let target = workspace::resolve(scope, facts, resolved)?;
    begin(&target, ts)
}

pub fn commit_for_scope(
    scope: workspace::Scope,
    facts: &workspace::Facts,
    resolved: &crate::config::Resolved,
    revision: &str,
) -> Result<CommitReport> {
    let target = workspace::resolve(scope, facts, resolved)?;
    commit(&target, revision)
}
