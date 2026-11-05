//! `distill --commit` semantics: install, drift refusal, replay noop,
//! revision id validation, immutable revisions.

use crate::common::{Uw, at, begin_distill, repo_env, resolve_repo_path};
use predicates::str::contains;

#[test]
pub(crate) fn distill_commit_installs_distilled_output_and_keeps_snapshot() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "stale fact"])
        .assert()
        .success();
    let target = resolve_repo_path(&home, &repo);
    let pre = std::fs::read_to_string(&target).uw();

    let (working, revision) = begin_distill(&home, &repo, "2026-01-05T00:00:00Z");

    // Agent rewrites: keep the entry, drop nothing, add a fresh fact.
    std::fs::write(&working, "distilled content\n").uw();
    at(&home, &repo, "2026-01-05T00:00:00Z")
        .args([
            "distill",
            "repo",
            "--commit",
            "--revision",
            &revision,
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"phase\":\"commit\""))
        .stdout(contains("\"noop\":false"));

    let after = std::fs::read_to_string(&target).uw();
    assert_eq!(after, "distilled content\n");

    // Snapshot immutable and retained, equal to the pre-distill content.
    let rev_dir = target.parent().uw().join("revisions").join(&revision);
    assert_eq!(
        std::fs::read_to_string(rev_dir.join("snapshot.md")).uw(),
        pre
    );
}

#[test]
pub(crate) fn distill_commit_refuses_on_drift() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "fact one"])
        .assert()
        .success();

    let (working, revision) = begin_distill(&home, &repo, "2026-01-05T00:00:00Z");

    // Concurrent append after --begin → drift.
    at(&home, &repo, "2026-01-06T00:00:00Z")
        .args(["append", "repo", "--text", "concurrent fact"])
        .assert()
        .success();
    std::fs::write(&working, "distilled content\n").uw();

    let before = std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw();
    at(&home, &repo, "2026-01-06T00:00:00Z")
        .args(["distill", "repo", "--commit", "--revision", &revision])
        .assert()
        .failure()
        .stderr(contains("changed since --begin"));
    let after = std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw();
    assert_eq!(before, after, "live file must be untouched on drift");
}

#[test]
pub(crate) fn distill_commit_rejects_crafted_revision_ids() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "fact"])
        .assert()
        .success();
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args([
            "distill",
            "repo",
            "--commit",
            "--revision",
            "../../evil-12345678",
        ])
        .assert()
        .failure()
        .stderr(contains("invalid revision id"));

    // Refusal must be side-effect free: target untouched, no revisions dir.
    let target = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&target).uw();
    assert!(content.ends_with(" fact\n"), "entry altered: {content:?}");
    assert!(!content.contains("evil"));
    assert!(!target.parent().uw().join("revisions").exists());
}

#[test]
pub(crate) fn distill_commit_replay_is_a_noop() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "fact"])
        .assert()
        .success();

    let (working, revision) = begin_distill(&home, &repo, "2026-01-05T00:00:00Z");
    std::fs::write(&working, "distilled\n").uw();
    at(&home, &repo, "2026-01-05T00:00:00Z")
        .args(["distill", "repo", "--commit", "--revision", &revision])
        .assert()
        .success();

    let out = String::from_utf8(
        at(&home, &repo, "2026-01-05T00:00:00Z")
            .args([
                "distill",
                "repo",
                "--commit",
                "--revision",
                &revision,
                "--json",
            ])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert!(out.contains("\"noop\":true"), "{out}");
    assert_eq!(
        std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw(),
        "distilled\n",
        "replay must not rewrite the live file"
    );
}

#[test]
pub(crate) fn distill_commit_without_working_file_fails() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "fact"])
        .assert()
        .success();

    let (_working, revision) = begin_distill(&home, &repo, "2026-01-05T00:00:00Z");

    at(&home, &repo, "2026-01-05T00:00:00Z")
        .args(["distill", "repo", "--commit", "--revision", &revision])
        .assert()
        .failure()
        .stderr(contains("working file not written"));
    // live file untouched
    assert!(
        std::fs::read_to_string(resolve_repo_path(&home, &repo))
            .uw()
            .contains("fact")
    );
}

#[test]
pub(crate) fn distill_two_cycles_produce_two_distinct_immutable_revisions() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "v1"])
        .assert()
        .success();
    let target = resolve_repo_path(&home, &repo);
    let pre = std::fs::read_to_string(&target).uw();

    let run_cycle = |now: &str, text: &str| -> String {
        let (working, revision) = begin_distill(&home, &repo, now);
        std::fs::write(&working, text).uw();
        at(&home, &repo, now)
            .args(["distill", "repo", "--commit", "--revision", &revision])
            .assert()
            .success();
        revision
    };

    let rev1 = run_cycle("2026-01-05T00:00:00Z", "first distill\n");
    let rev2 = run_cycle("2026-01-06T00:00:00Z", "second distill\n");
    assert_ne!(rev1, rev2);
    let rev_dir = target.parent().uw().join("revisions");
    assert!(rev_dir.join(&rev1).join("snapshot.md").exists());
    assert!(rev_dir.join(&rev2).join("snapshot.md").exists());
    assert_eq!(
        std::fs::read_to_string(rev_dir.join(&rev1).join("snapshot.md")).uw(),
        pre
    );
    assert_eq!(std::fs::read_to_string(&target).uw(), "second distill\n");
}
