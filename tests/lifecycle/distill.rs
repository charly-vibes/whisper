//! `distill --begin` semantics: snapshots, paths, linting, recency.

use crate::common::{Uw, at, extract_id, repo_env, resolve_repo_path};
use predicates::str::contains;

#[test]
pub(crate) fn distill_begin_exposes_entry_recency() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "recency tracked fact"])
        .assert()
        .success();
    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args(["recall", "repo", "--json"])
        .assert()
        .success();

    let out = String::from_utf8(
        at(&home, &repo, "2026-01-05T00:00:00Z")
            .args(["distill", "repo", "--begin", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert!(
        out.contains("\"recency\""),
        "begin must expose per-entry recency: {out}"
    );
    let data: serde_json::Value = serde_json::from_str(&out).uw();
    let recency = data["data"]["recency"]
        .as_object()
        .expect("recency must be an object keyed by entry id");
    let id = extract_id(&resolve_repo_path(&home, &repo), "recency tracked fact");
    let r = &recency[&id];
    assert_eq!(r["use_count"].as_u64().uw(), 1);
    assert_eq!(r["last_recalled"].as_str().uw(), "2026-01-02T00:00:00Z");
}

#[test]
pub(crate) fn distill_begin_lints_entry_texts_not_entry_markers() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    // Well-formed entries always carry [id:…] lines — lint must read the
    // entry TEXT, not the raw file, or every file would warn.
    let out = at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "a calm rule"])
        .assert()
        .success();
    assert_eq!(out.get_output().status.code(), Some(0));
    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "hotfixed 2026-09-28 via PR #44 commit b1a2c3d4 rerun passed",
        ])
        .assert()
        .success();

    at(&home, &repo, "2026-01-05T00:00:00Z")
        .args(["distill", "repo", "--begin", "--json"])
        .assert()
        .success()
        .stdout(contains("lessons, not logs"));
}

#[test]
pub(crate) fn distill_begin_refuses_when_no_scope_file() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let out = at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["distill", "repo", "--begin"])
        .assert()
        .failure();
    assert_eq!(out.get_output().status.code(), Some(1));
}

#[test]
pub(crate) fn distill_begin_snapshots_and_returns_paths() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "stale fact"])
        .assert()
        .success();

    let out = String::from_utf8(
        at(&home, &repo, "2026-01-05T00:00:00Z")
            .args(["distill", "repo", "--begin", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert!(out.contains("\"phase\":\"begin\""));
    assert!(out.contains("\"revision\":\"20260105T000000Z-"));
    assert!(out.contains("working.md\""));
    assert!(out.contains("snapshot.md\""));
    // snapshot + meta exist; revisions dir lives next to the target file
    assert!(out.contains("revisions"));
}
