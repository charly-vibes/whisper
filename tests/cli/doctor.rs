//! doctor integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};

use predicates::str::contains;

#[test]
fn doctor_reports_unhealthy_then_healthy_after_sync() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = doctor_cmd(&tmp, &repo)
        .stdout(contains("turu.managed-block"))
        .stdout(contains("\"warn\":5"));
    assert_eq!(out.get_output().status.code(), Some(0));

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    doctor_cmd(&tmp, &repo)
        .stdout(contains("\"pass\":13"))
        .stdout(contains("\"warn\":0"));
}

/// `turu doctor --json` run for this checkout (exit 0 expected).
fn doctor_cmd(tmp: &tempfile::TempDir, repo: &std::path::Path) -> assert_cmd::assert::Assert {
    turu(tmp.path(), repo).args(["doctor", "--json"]).assert()
}

#[test]
fn doctor_warns_when_repo_private_root_shadows_global_rules() {
    // GH-issue follow-up: a repo-private `workspace_root` intentionally beats
    // everything — which also relocates the global scope (`rules.md`) for
    // that repo. Doctor must surface the shadowing and any divergence.
    let tmp = tempfile::tempdir().uw();
    let (home, repo, private_ws) = shadowed_fixture(&tmp);

    // 1. Global rules.md exists but is hidden by the private root → warn.
    doctor(&home, &repo)
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("hides"));

    // 2. Identical content in both → informational pass.
    std::fs::create_dir_all(&private_ws).uw();
    std::fs::write(private_ws.join("rules.md"), "global rule\n").uw();
    doctor(&home, &repo)
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("in sync"));

    // 3. Diverged content → warn.
    std::fs::write(private_ws.join("rules.md"), "diverged\n").uw();
    let out = doctor(&home, &repo)
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("diverged"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// Fixture: global workspace with rules.md + a repo-private root override
/// that shadows it. Returns (home, repo, private_ws).
fn doctor(home: &std::path::Path, repo: &std::path::Path) -> assert_cmd::assert::Assert {
    turu(home, repo).args(["doctor", "--json"]).assert()
}

fn shadowed_fixture(
    tmp: &tempfile::TempDir,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let global_ws = tmp.path().join("global-ws");
    std::fs::create_dir_all(&global_ws).uw();
    std::fs::write(global_ws.join("rules.md"), "global rule\n").uw();
    std::fs::create_dir_all(home.join(".config/whisper")).uw();
    std::fs::write(
        home.join(".config/whisper/config.toml"),
        format!("workspace_root = '{}'\n", global_ws.display()),
    )
    .uw();
    let private_ws = tmp.path().join("private-ws");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!("workspace_root = '{}'\n", private_ws.display()),
    )
    .uw();
    (home, repo, private_ws)
}

#[test]
fn check_warns_when_repo_private_root_shadows_global_rules() {
    // whisper-122 follow-through: `turu check` is the fast-path validator —
    // it must surface the shadowing alongside `turu doctor`.
    let tmp = tempfile::tempdir().uw();
    let (home, repo, private_ws) = shadowed_fixture(&tmp);

    check(&home, &repo).stdout(contains("shadows the global rules file"));

    // In-sync copies do not warn.
    std::fs::create_dir_all(&private_ws).uw();
    std::fs::write(private_ws.join("rules.md"), "global rule\n").uw();
    let out = check(&home, &repo).stdout(contains("workspace looks consistent"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// `turu check --json` for this checkout (exit 0 expected).
fn check(home: &std::path::Path, repo: &std::path::Path) -> assert_cmd::assert::Assert {
    turu(home, repo).args(["check", "--json"]).assert()
}

#[test]
fn check_flags_missing_rules_file() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["check", "--json"])
        .assert()
        .success()
        .stdout(contains("rules file missing"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

#[test]
fn check_flags_legacy_variants() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    let legacy = tmp.path().join(".whisper/repos/r");
    std::fs::create_dir_all(&legacy).uw();
    std::fs::write(legacy.join("env.md"), "legacy fact\n").uw();

    let out = turu(tmp.path(), &repo)
        .args(["check", "--json"])
        .assert()
        .success()
        .stdout(contains("legacy repo-key variants"))
        .stdout(contains("github.com/u/r"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

#[test]
fn doctor_reports_routing_mode_and_flags_stale_checkout_layer() {
    let (tmp, repo) = machine_local_fixture();

    // Clean switch: no checkout layer content → pass, store route reported.
    doctor_routing(&tmp, &repo)
        .stdout(contains("turu.routing"))
        .stdout(contains("machine-local store"))
        .stdout(contains("\"warn\":0"));

    // Leftover checkout-layer content → advisory warn naming the files.
    leave_leftover(&repo);

    let out = doctor_routing(&tmp, &repo)
        .stdout(contains("turu.routing"))
        .stdout(contains("checkout layer still carries content"))
        .stdout(contains("\"warn\":1"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

/// `turu doctor --json` for this checkout (exit 0 expected).
fn doctor_routing(tmp: &tempfile::TempDir, repo: &std::path::Path) -> assert_cmd::assert::Assert {
    turu(tmp.path(), repo).args(["doctor", "--json"]).assert()
}

/// Fixture: repo configured for machine-local routing, initialized and
/// synced (store layer provisioned, managed block wired).
fn machine_local_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "routing = \"machine-local\"\n",
    )
    .uw();
    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    (tmp, repo)
}

/// Write a leftover checkout-layer env.md (pre-switch leftover).
fn leave_leftover(repo: &std::path::Path) {
    std::fs::write(
        repo.join(".whisper/env.md"),
        "leftover from the previous mode\n",
    )
    .uw();
}
