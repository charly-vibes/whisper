//! Bundle pack/unpack semantics: determinism, roundtrip, extend-only unpack.

use crate::common::{
    Uw, extract_id, git_repo, pack_bundle, repo_env, resolve_repo_path, seed_entries, turu,
    unpack_bundle,
};
use predicates::str::contains;

#[test]
pub(crate) fn bundle_pack_creates_missing_parent_dirs_for_out() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "fact"])
        .assert()
        .success();
    let nested = tmp.path().join("deep/nested/dir/bundle.json");
    pack_bundle(&home, &repo, &nested);
    assert!(nested.exists());
}

#[test]
pub(crate) fn bundle_pack_group_without_active_group_fails() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let out = turu(&home, &repo)
        .args(["bundle", "pack", "group"])
        .assert()
        .failure()
        .stderr(contains("no group is active"));
    assert_eq!(out.get_output().status.code(), Some(1));
}

#[test]
pub(crate) fn bundle_pack_is_byte_deterministic() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    seed_entries(&home, &repo);
    let out1 = tmp.path().join("b1.json");
    let out2 = tmp.path().join("b2.json");
    for out in [&out1, &out2] {
        pack_bundle(&home, &repo, out);
    }
    assert_eq!(
        std::fs::read(&out1).uw(),
        std::fs::read(&out2).uw(),
        "identical state must pack to identical bytes"
    );
}

#[test]
pub(crate) fn bundle_roundtrip_into_a_fresh_workspace_keeps_ids() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    seed_entries(&home, &repo);
    let bundle_path = tmp.path().join("bundle.json");
    pack_bundle(&home, &repo, &bundle_path);

    // Fresh workspace: a different repo with no entries.
    let repo_b = tmp.path().join("repo-b");
    git_repo(&repo_b, "git@github.com:other/repo-b.git");
    let home_b = tmp.path().join("home-b");
    std::fs::create_dir_all(&home_b).uw();

    let out = unpack_bundle(&home_b, &repo_b, &bundle_path);
    assert!(out.contains("\"added\":3"), "{out}");
    assert!(out.contains("\"duplicates_skipped\":0"), "{out}");

    // Same entry ids after the round trip.
    let content_b = std::fs::read_to_string(resolve_repo_path(&home_b, &repo_b)).uw();
    for text in ["oldest fact", "middle fact", "newest fact"] {
        let id_a = extract_id(&resolve_repo_path(&home, &repo), text);
        let line_b = content_b.lines().find(|l| l.contains(text)).uw();
        assert!(line_b.contains(&id_a), "entry id preserved for {text}");
    }
}

#[test]
pub(crate) fn bundle_unpack_is_extend_only_and_skips_duplicates() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    seed_entries(&home, &repo);
    let bundle_path = tmp.path().join("bundle.json");
    pack_bundle(&home, &repo, &bundle_path);

    // Add local-only content, then unpack the bundle over it.
    turu(&home, &repo)
        .env("TURU_NOW", "2026-03-01T00:00:00Z")
        .args(["append", "repo", "--text", "local only fact"])
        .assert()
        .success();
    let target = resolve_repo_path(&home, &repo);
    let before = std::fs::read_to_string(&target).uw();

    let out = unpack_bundle(&home, &repo, &bundle_path);
    assert!(out.contains("\"added\":0"), "{out}");
    assert!(out.contains("\"duplicates_skipped\":3"), "{out}");

    let after = std::fs::read_to_string(&target).uw();
    assert_eq!(
        before, after,
        "unpack with only duplicates must not rewrite"
    );
    assert!(after.contains("local only fact"));
}
