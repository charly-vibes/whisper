//! key integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;

#[test]
fn key_is_deterministic_and_canonical() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"cv/charly-vibes/whisper\""));
    assert_eq!(out.get_output().status.code(), Some(0));

    // Same repo → same key, on repeat invocations.
    turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"cv/charly-vibes/whisper\""));
}
#[test]
fn https_remote_maps_to_host_key() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");

    let out = turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"github.com/u/r\""));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn no_remote_falls_back_to_local_key() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("plain");
    git_repo(&repo, "");

    let out = turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"local/plain\""));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn repo_local_layout_matches_the_spec() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/env.md"));
    assert_eq!(out.get_output().status.code(), Some(0));

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/branches/main/notes.md"));
}
