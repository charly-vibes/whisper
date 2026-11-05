//! feedback integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;

#[test]
fn feedback_dry_run_previews_body_and_fallback_url() {
    let tmp = tempfile::tempdir().uw();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(&home, &repo)
        .args(["feedback", "bug", "--dry-run"])
        .write_stdin("steps to reproduce the bug\n")
        .assert()
        .success()
        .stdout(contains("fallback"))
        .stdout(contains("issues/new"))
        .stderr(contains("## Description"))
        .stderr(contains("steps to reproduce the bug"))
        .stderr(contains("Would file: gh issue create"));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn feedback_rejects_unknown_kind() {
    let tmp = tempfile::tempdir().uw();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(&home, &repo)
        .args(["feedback", "bugz"])
        .assert()
        .failure()
        .stdout(contains("unknown kind"))
        .stdout(contains("pass one of: bug, feature, question, chore"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn feedback_requires_content() {
    let tmp = tempfile::tempdir().uw();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Closed stdin (not a terminal) and no --from-last-error: loud failure.
    let out = turu(&home, &repo)
        .args(["feedback", "bug"])
        .assert()
        .failure()
        .stdout(contains("No issue content specified"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn feedback_from_last_error_reads_the_scratch() {
    let tmp = tempfile::tempdir().uw();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let cache = tmp.path().join("cache");

    // 1. A failing turu command persists an error-scratch record (best-effort).
    turu(&home, &repo)
        .env("XDG_CACHE_HOME", &cache)
        .args(["resolve", "group", "--json"])
        .assert()
        .failure();
    assert!(cache.join("turu/errors.jsonl").exists());

    // 2. `feedback bug --from-last-error` folds that record into the body.
    turu(&home, &repo)
        .env("XDG_CACHE_HOME", &cache)
        .args(["feedback", "bug", "--from-last-error", "--dry-run"])
        .assert()
        .success()
        .stderr(contains("## Error"))
        .stderr(contains("auto-reported error"))
        .stdout(contains("fallback"));
}
