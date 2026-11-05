//! append integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;

#[test]
fn append_creates_then_extends_verbatim() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["append", "repo", "--text", "deploy needs vault"])
        .assert()
        .success();

    turu(tmp.path(), &repo)
        .args(["append", "repo", "--text", "second fact"])
        .assert()
        .success();

    let env_md = repo.join(".whisper/env.md");
    let content = std::fs::read_to_string(&env_md).uw();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    for (line, text) in lines.iter().zip(["deploy needs vault", "second fact"]) {
        assert!(line.starts_with("- 20") && line.contains(" [id:"));
        assert!(line.ends_with(text));
    }
}
#[test]
fn append_accepts_positional_text() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // The form agents naturally write: text as a positional argument.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch", "positional fact"])
        .assert()
        .success()
        .stdout(contains("\"appended_bytes\""));

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).uw();
    assert!(notes.contains("positional fact"));
}
#[test]
fn append_reads_piped_stdin_without_stdin_flag() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Piped stdin is auto-detected: no --stdin flag needed.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch"])
        .write_stdin("piped fact\n")
        .assert()
        .success();

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).uw();
    assert!(notes.contains("piped fact"));
}
#[test]
fn append_stdin_flag_before_scope_still_works() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Flag-order tolerance: --stdin before the positional scope.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "--stdin", "branch"])
        .write_stdin("ordered fact\n")
        .assert()
        .success();

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).uw();
    assert!(notes.contains("ordered fact"));
}
#[test]
fn append_empty_piped_stdin_still_errors() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let out = turu(tmp.path(), &repo)
        .args(["append", "branch"])
        .write_stdin("")
        .assert()
        .failure()
        .stderr(contains("nothing to append"));
    assert_eq!(out.get_output().status.code(), Some(1));
}
#[test]
fn append_positional_text_wins_over_piped_stdin() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // When both are present, the explicit text is the payload; stdin is
    // never consumed (no hang, no double append).
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch", "explicit fact"])
        .write_stdin("ignored fact\n")
        .assert()
        .success();

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).uw();
    assert!(notes.contains("explicit fact"));
    assert!(!notes.contains("ignored fact"));
}

// --- one-call recall loop (whisper-s8r): 'recall all' composes precedence ---
#[test]
fn pack_excludes_the_private_zone_and_reports_the_count() {
    let (tmp, repo) = seeded_private_fixture();
    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["bundle", "pack", "branch", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    assert!(out.contains("committed fact"));
    assert!(
        !out.contains("machine-secret"),
        "private text in bundle: {out}"
    );
    assert!(out.contains("\"private_excluded\":1"), "{out}");
}
#[test]
fn recall_serves_the_private_zone_store_major() {
    let (tmp, repo) = layered_branch_fixture();
    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["recall", "branch", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    let a = out.find("alpha private").uw();
    let b = out.find("bravo committed").uw();
    let c = out.find("charlie store").uw();
    assert!(
        b < a && a < c,
        "checkout entries first (recency inside), then store entries; {out}"
    );
}
#[test]
fn recall_composes_store_major_with_dedup() {
    let (tmp, repo) = store_newer_than_checkout_fixture();
    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["recall", "repo", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    let a = out.find("carol alpha").uw();
    let b = out.find("bravo alpha").uw();
    let c = out.find("alpha store").uw();
    assert!(
        b < a && a < c,
        "store-major: checkout entries first (recency inside), then store entries; {out}"
    );
    // No id dedup false-positive: each entry appears exactly once.
    assert_eq!(out.matches("alpha").count(), 3);
}
#[test]
fn recall_all_composes_global_repo_and_branch() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // One entry per layer, distinct timestamps.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "global", "global layer fact"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args(["append", "repo", "repo layer fact"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-03T00:00:00Z")
        .args(["append", "branch", "branch layer fact"])
        .assert()
        .success();

    // One call serves every layer — the session-start read.
    let out = turu(tmp.path(), &repo)
        .args(["recall", "all", "--json"])
        .output()
        .uw();
    assert!(out.status.success());
    let json = String::from_utf8_lossy(&out.stdout);
    // Served via the envelope's entries: all three layers present.
    assert!(json.contains("global layer fact"));
    assert!(json.contains("repo layer fact"));
    assert!(json.contains("branch layer fact"));
}

// ---------------------------------------------------------------------------
// Deployed-spec contract bindings (whisper-15z): scenarios that had no
// executable verification. Each test maps 1:1 to one .espectacular contract.
// ---------------------------------------------------------------------------

#[test]
fn same_text_in_both_layers_stays_two_distinct_entries() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    for global in [false, true] {
        let mut cmd = turu(tmp.path(), &repo);
        cmd.env("TURU_NOW", "2026-01-01T00:00:00Z");
        cmd.args(["append", "repo", "--text", "stated twice"]);
        if global {
            cmd.arg("--global");
        }
        cmd.assert().success();
    }

    // Same scope, same second, same text — but different id spaces, so both
    // entries exist and recall (which dedups by id) serves them both.
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
fn store_packed_bundle_unpacks_into_the_repo_local_root() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Pre-decision-A style: pack the workspace-root store (done here from
    // outside any checkout, where repo scope resolves to the store).
    turu(tmp.path(), tmp.path())
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "legacy packed fact"])
        .assert()
        .success();
    turu(tmp.path(), tmp.path())
        .args(["bundle", "pack", "repo", "--out", "bundle.json", "--json"])
        .assert()
        .success();

    // Unpack in the checkout: entries land at the current-resolution path
    // (the repo-local root), extend-never-overwrite unchanged.
    turu(tmp.path(), &repo)
        .args(["bundle", "unpack", "--file", "../bundle.json", "--json"])
        .assert()
        .success()
        .stdout(contains("\"added\":1"));
    let landed = std::fs::read_to_string(repo.join(".whisper/env.md")).uw();
    assert!(landed.contains("legacy packed fact"), "{landed}");
}

/// Fixture: checkout + private layer + a `privateX` decoy that is
/// ordinary content (exact-path rule) — one committed, one private entry.
fn seeded_private_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch", "--text", "committed fact"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args(["append", "branch", "--private", "--text", "machine-secret"])
        .assert()
        .success();
    let decoy = repo.join(".whisper/privateX/branches/main");
    std::fs::create_dir_all(&decoy).uw();
    std::fs::write(decoy.join("notes.md"), "- decoy entry\n").uw();
    (tmp, repo)
}

/// Fixture: private + committed checkout entries, then a newer store
/// entry — store-major recall order across all three layers.
fn layered_branch_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    for (day, flag, text) in [
        ("2026-01-01T00:00:00Z", "--private", "alpha private"),
        ("2026-01-02T00:00:00Z", "", "bravo committed"),
        ("2026-01-03T00:00:00Z", "--global", "charlie store"),
    ] {
        turu(tmp.path(), &repo)
            .env("TURU_NOW", day)
            .args(["append", "branch", flag, "--text", text])
            .assert()
            .success();
    }
    (tmp, repo)
}

/// Fixture: repo-local entries (older) written in the checkout, then a
/// NEWER machine-local-store entry (the --global escape hatch).
fn store_newer_than_checkout_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    for (day, flag, text) in [
        ("2026-01-01T00:00:00Z", "", "carol alpha"),
        ("2026-01-02T00:00:00Z", "", "bravo alpha"),
        ("2026-01-03T00:00:00Z", "--global", "alpha store"),
    ] {
        turu(tmp.path(), &repo)
            .env("TURU_NOW", day)
            .args(["append", "repo", flag, "--text", text])
            .assert()
            .success();
    }
    (tmp, repo)
}
