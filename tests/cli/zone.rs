//! `--private` zone routing tests (decision A: zone is checkout-local).

use super::common::Uw;
use crate::common::{git_repo, gitignore_rule_count, turu};
use std::process::Command;

use predicates::str::contains;

#[test]
fn private_flag_routes_into_the_zone_mirroring_the_layout() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--private", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/private/env.md"));

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--private", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/private/branches/main/notes.md"));

    turu(tmp.path(), &repo)
        .args(["append", "branch", "--private", "--text", "machine-secret"])
        .assert()
        .success();
    assert!(
        repo.join(".whisper/private/branches/main/notes.md")
            .exists()
    );
    // A private append touches neither the committed repo-local file nor
    // the machine-local store.
    assert!(!repo.join(".whisper/branches/main/notes.md").exists());
    assert!(
        !tmp.path()
            .join(".whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md")
            .exists()
    );
}

#[test]
fn private_and_committed_layers_keep_distinct_id_spaces() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    for private in [false, true] {
        let mut cmd = turu(tmp.path(), &repo);
        cmd.env("TURU_NOW", "2026-01-01T00:00:00Z");
        cmd.args(["append", "repo", "--text", "stated twice"]);
        if private {
            cmd.arg("--private");
        }
        cmd.assert().success();
    }

    // Same scope, same second, same text — but distinct id spaces (like
    // the @local/@store split), so recall's id dedup serves both.
    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["recall", "repo", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert_eq!(out.matches("stated twice").count(), 2);
}

#[test]
fn private_zone_is_exact_path_only() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // A `private/` directory nested deeper under .whisper/ is ordinary
    // content: recall never searches, so it is not served. Resolution is
    // exact-path (`.whisper/private/` only), never a name match.
    let nested = repo.join(".whisper/branches/main/private");
    std::fs::create_dir_all(&nested).uw();
    std::fs::write(nested.join("notes.md"), "nested private line\n").uw();

    let out = turu(tmp.path(), &repo)
        .args(["recall", "branch", "--json"])
        .output()
        .uw();
    let stdout = String::from_utf8(out.stdout).uw();
    // Nothing servable in this scope — the nested dir is invisible, so the
    // recall is ok:true-empty (whisper-2c1 semantics), never an error.
    assert!(stdout.contains("\"empty\":true"), "{stdout}");
    assert!(!stdout.contains("nested private line"), "{stdout}");
}

#[test]
pub(crate) fn doctor_warns_when_private_zone_is_not_effectively_ignored() {
    let (tmp, repo) = bare_repo_fixture();

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-ignore"))
        .stdout(contains("not effectively ignored"));

    // Self-heal: init appends the rule exactly once, and the check passes.
    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    assert_eq!(gitignore_rule_count(&repo), 1);
    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-ignore"))
        .stdout(contains("\"warn\":0"));
}

/// An empty git repo with no workspace yet (doctor warns on it).
fn bare_repo_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    (tmp, repo)
}

#[test]
pub(crate) fn doctor_fails_prominently_on_tracked_files_under_private_zone() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    std::fs::create_dir_all(repo.join(".whisper/private")).uw();
    std::fs::write(
        repo.join(".whisper/private/env.md"),
        "secret machine facts\n",
    )
    .uw();
    let status = Command::new("git")
        .args(["add", "-f", ".whisper/private/env.md"])
        .current_dir(&repo)
        .status()
        .uw();
    assert!(status.success());

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-tracked"))
        .stdout(contains("\"fail\":1"))
        .stdout(contains("history is forever"));
}

#[test]
pub(crate) fn doctor_emits_leak_warnings_for_public_repo_local_files() {
    let (tmp, repo) = leaky_repo_fixture();

    // Advisory: doctor still exits 0; the leak surfaces in warnings[].
    let out = turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-leaks"))
        .stdout(contains("machine path"))
        .stdout(contains("\"warn\":1"))
        .stdout(contains("\"fail\":0"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// Fixture: synced checkout with a machine-path fact in the public
/// repo-local env.md (advisory leak lint fodder).
fn leaky_repo_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    turu(tmp.path(), &repo)
        .args([
            "append",
            "repo",
            "--text",
            "deploy runs from /home/sasha/infra",
        ])
        .assert()
        .success();
    (tmp, repo)
}

#[test]
pub(crate) fn doctor_leak_lint_stays_quiet_on_clean_checkouts() {
    let (tmp, repo) = clean_repo_fixture();

    let out = turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-leaks"))
        .stdout(contains("\"warn\":0"))
        .stdout(contains("\"fail\":0"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// Fixture: synced checkout whose one branch note is leak-free.
fn clean_repo_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let out = turu(tmp.path(), &repo).args(["init"]).assert().success();
    assert_eq!(out.get_output().status.code(), Some(0));
    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    turu(tmp.path(), &repo)
        .args([
            "append",
            "branch",
            "--text",
            "prefer O_APPEND fast path over full rewrites",
        ])
        .assert()
        .success();
    (tmp, repo)
}

#[test]
pub(crate) fn doctor_leak_lint_skips_superseded_entries() {
    let (tmp, repo, _id) = superseded_leak_fixture();

    let out = turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-leaks"))
        .stdout(contains("\"warn\":0"))
        .stdout(contains("\"fail\":0"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// Fixture: synced checkout whose leaky entry was superseded by a clean
/// one (superseded = dead by decision, cf. usage-staleness).
fn superseded_leak_fixture() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let (tmp, repo) = leaky_repo_root();
    init_and_sync(&tmp, &repo);
    let id = append_leaky_entry(&tmp, &repo);
    supersede_with_clean(&tmp, &repo, &id);
    (tmp, repo, id)
}

/// `turu init` + `turu sync` (provisions layout and private-ignore rule).
fn init_and_sync(tmp: &tempfile::TempDir, repo: &std::path::Path) {
    turu(tmp.path(), repo).args(["init"]).assert().success();
    turu(tmp.path(), repo).args(["sync"]).assert().success();
}

/// Supersede the leaky entry with clean text.
fn supersede_with_clean(tmp: &tempfile::TempDir, repo: &std::path::Path, id: &str) {
    turu(tmp.path(), repo)
        .args([
            "append",
            "repo",
            "--json",
            "--supersedes",
            id,
            "--text",
            "deploy needs vault login",
        ])
        .assert()
        .success();
}

/// Append the leaky entry, returning its entry id.
fn append_leaky_entry(tmp: &tempfile::TempDir, repo: &std::path::Path) -> String {
    let out = turu(tmp.path(), repo)
        .args([
            "append",
            "repo",
            "--json",
            "--text",
            "deploy runs from /home/sasha/infra",
        ])
        .output()
        .uw();
    let envelope: serde_json::Value = serde_json::from_slice(&out.stdout).uw();
    let id = envelope["data"]["id"].as_str().uw().to_string();
    assert_eq!(id.len(), 64);
    id
}

/// Temp dir + git repo for the superseded-leak fixture.
fn leaky_repo_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    (tmp, repo)
}
