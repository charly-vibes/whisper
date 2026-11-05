//! resolve_routing integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;

use std::process::Command;
#[test]
fn resolve_branch_lands_in_the_checkout() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/branches/main/notes.md"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

// whisper-6fv — decision A of add-repo-private-scope: repo/branch knowledge
// lives in the checkout's own .whisper/ (travels with git); the machine-local
// store stays servable via recall composition and reachable via --global.
#[test]
fn global_flag_rejected_outside_repo_and_branch_scopes() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["append", "worktree", "--global", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--global applies only to the repo and branch scopes",
        ));
    assert_eq!(out.get_output().status.code(), Some(1));

    turu(tmp.path(), &repo)
        .args(["append", "global", "--global", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--global applies only to the repo and branch scopes",
        ));
}
#[test]
fn private_flag_requires_a_checkout() {
    let tmp = tempfile::tempdir().uw();
    // cwd is not a git repo: no repo-local root, no private zone.
    let out = turu(tmp.path(), tmp.path())
        .args(["append", "repo", "--private", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains("--private requires a git checkout"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn private_flag_rejected_like_the_escape_hatch() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["append", "worktree", "--private", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--private applies only to the repo and branch scopes",
        ));
    assert_eq!(out.get_output().status.code(), Some(1));

    turu(tmp.path(), &repo)
        .args(["append", "global", "--private", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--private applies only to the repo and branch scopes",
        ));

    turu(tmp.path(), &repo)
        .args(["append", "repo", "--private", "--global", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains("--global and --private are mutually exclusive"));
}
#[test]
fn routing_mode_machine_local_routes_the_store() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "routing = \"machine-local\"\n",
    )
    .uw();

    // repo/branch resolve to the machine-local store by default — the
    // persistent equivalent of the --global escape hatch.
    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains(".whisper/repos/github.com/u/r/env.md"))
        .stdout(contains("\"exists\":false"));

    turu(tmp.path(), &repo)
        .args(["append", "branch", "store by default"])
        .assert()
        .success();
    assert!(
        tmp.path()
            .join(".whisper/repos/github.com/u/r/branches/main/notes.md")
            .exists(),
        "branch write lands in the machine-local store"
    );
    assert!(
        !repo.join(".whisper/branches/main/notes.md").exists(),
        "checkout layer stays untouched under machine-local routing"
    );
}
#[test]
fn routing_mode_repo_local_is_the_default() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();

    // No routing key → decision-A behavior: repo/branch in the checkout.
    let out = turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/env.md"));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn routing_mode_rejects_unknown_values() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(repo.join(".whisper/config.toml"), "routing = \"cloud\"\n").uw();

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .failure()
        .stderr(contains("unknown routing mode 'cloud'"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn routing_mode_machine_local_still_honors_the_zone() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "routing = \"machine-local\"\n",
    )
    .uw();

    // The --private zone is an explicit override and stays checkout-local.
    turu(tmp.path(), &repo)
        .args(["append", "branch", "--private", "private fact"])
        .assert()
        .success();
    assert!(
        repo.join(".whisper/private/branches/main/notes.md")
            .exists()
    );
}
#[test]
fn group_scope_without_group_fails_with_suggestion() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .failure()
        .stderr(contains("no group is active"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn unchanged_scopes_keep_their_store_destinations() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "global", "--json"])
        .assert()
        .success()
        .stdout(contains(".whisper/rules.md"));
    assert_eq!(out.get_output().status.code(), Some(0));

    turu(tmp.path(), &repo)
        .args(["resolve", "worktree", "--json"])
        .assert()
        .success()
        .stdout(contains(
            ".whisper/repos/cv/charly-vibes/whisper/worktrees/.git/env.md",
        ));
}
#[test]
fn feature_branch_slug_is_double_dashed() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    let status = Command::new("git")
        .args(["checkout", "-b", "feature/login-fix"])
        .current_dir(&repo)
        .status()
        .uw();
    assert!(status.success(), "branch created");

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains("branches/feature--login-fix/notes.md"));
}
#[test]
fn linked_worktree_shares_the_slot() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    let worktree = tmp.path().join("wt");
    let status = Command::new("git")
        .args([
            "worktree",
            "add",
            &worktree.to_string_lossy(),
            "-b",
            "wt-branch",
        ])
        .current_dir(&repo)
        .status()
        .uw();
    assert!(status.success(), "worktree added");

    // Main checkout and linked worktree share the common dir basename (.git).
    turu(tmp.path(), &repo)
        .args(["resolve", "worktree", "--json"])
        .assert()
        .success()
        .stdout(contains("worktrees/.git/env.md"));
    turu(tmp.path(), &worktree)
        .args(["resolve", "worktree", "--json"])
        .assert()
        .success()
        .stdout(contains("worktrees/.git/env.md"));
}
#[test]
fn bare_repo_uses_the_bare_dir_as_slot() {
    let tmp = tempfile::tempdir().uw();
    let bare = tmp.path().join("repo.git");
    let status = Command::new("git")
        .args(["init", "--bare", &bare.to_string_lossy()])
        .status()
        .uw();
    assert!(status.success(), "bare repo created");

    turu(tmp.path(), &bare)
        .args(["resolve", "worktree", "--json"])
        .assert()
        .success()
        .stdout(contains("worktrees/repo.git/env.md"));
}

#[test]
fn resolve_append_global_escape_hatch_writes_the_store() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--global", "--json"])
        .assert()
        .success()
        .stdout(contains(
            ".whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md",
        ));

    turu(tmp.path(), &repo)
        .args(["append", "repo", "--global", "--text", "stored fact"])
        .assert()
        .success();
    assert!(
        tmp.path()
            .join(".whisper/repos/cv/charly-vibes/whisper/env.md")
            .exists()
    );
    // The default destination stays untouched by a --global append.
    assert!(!repo.join(".whisper/env.md").exists());
}

#[test]
fn recall_serves_pre_existing_store_knowledge() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Knowledge written the pre-decision-A way, straight into the machine-
    // local store, must remain servable in the checkout (spec scenario).
    let out = turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--global", "--text", "legacy store fact"])
        .assert()
        .success();
    assert_eq!(out.get_output().status.code(), Some(0));

    turu(tmp.path(), &repo)
        .args(["recall", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("legacy store fact"));
}
