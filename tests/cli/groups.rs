//! groups integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;

#[test]
fn global_group_membership_routes_to_group_root() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).uw();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.shared]\nroot = \"{}\"\nrepos = [\"cv/charly-vibes/whisper\"]\n",
            tmp.path().join("shared-knowledge").display()
        ),
    )
    .uw();

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .success()
        .stdout(contains(
            "shared-knowledge/repos/cv/charly-vibes/whisper/env.md",
        ));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn repo_private_config_joins_group() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).uw();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.cv-tools]\nroot = \"{}\"\n",
            tmp.path().join("cv-root").display()
        ),
    )
    .uw();

    // Private, never-committed repo config.
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(repo.join(".whisper/config.toml"), "group = \"cv-tools\"\n").uw();

    let out = turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .success()
        .stdout(contains("cv-root/repos/cv/charly-vibes/whisper/env.md"));
    assert_eq!(out.get_output().status.code(), Some(0));
}
#[test]
fn repo_private_root_override_wins() {
    let (tmp, repo) = override_fixture();

    // The override relocates the machine-local store: the global scope's
    // rules.md (and the repo/branch --global destinations) move there.
    let out = turu(tmp.path(), &repo)
        .args(["resolve", "global", "--json"])
        .assert()
        .success()
        .stdout(contains("private-ws/rules.md"));
    assert_eq!(out.get_output().status.code(), Some(0));

    // The checkout's repo-local root is unaffected by the override —
    // decision A routes repo/branch knowledge into the checkout itself.
    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/env.md"));
}
#[test]
fn consolidate_leaves_the_checkout_private_zone_untouched() {
    let (tmp, repo, variant, decoy) = consolidate_private_fixture();
    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["consolidate", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert!(
        out.contains("repos/cv/charly-vibes/whisper"),
        "variant not migrated: {out}"
    );
    assert!(
        !variant.join("env.md").exists(),
        "legacy variant not consumed"
    );
    assert!(decoy.join("env.md").exists(), "private decoy must survive");
}

/// Fixture: knowledge root relocated into the checkout, with a real legacy
/// variant in the store and a look-alike decoy under the private zone.
fn consolidate_private_fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    // Relocate the knowledge root into the checkout (repo-private
    // workspace_root override) — the closest consolidate can legally get
    // to checkout-local knowledge today.
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!("workspace_root = '{}'\n", repo.join(".whisper").display()),
    )
    .uw();
    // A real legacy variant in the store, plus a decoy under the private
    // zone that looks exactly like one.
    let variant = repo.join(".whisper/repos/whisper");
    std::fs::create_dir_all(&variant).uw();
    std::fs::write(variant.join("env.md"), "legacy line\n").uw();
    let decoy = repo.join(".whisper/private/repos/whisper");
    std::fs::create_dir_all(&decoy).uw();
    std::fs::write(decoy.join("env.md"), "private line\n").uw();
    (tmp, repo, variant, decoy)
}

/// Fixture: global group config + repo-private workspace_root override.
fn override_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).uw();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.cv-tools]\nroot = \"{}\"\n",
            tmp.path().join("cv-root").display()
        ),
    )
    .uw();
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!(
            "workspace_root = \"{}\"\n",
            tmp.path().join("private-ws").display()
        ),
    )
    .uw();
    (tmp, repo)
}
