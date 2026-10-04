//! End-to-end CLI tests: determinism of key derivation, scope resolution,
//! append semantics, and config precedence.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as CliCommand;
use predicates::str::contains;

/// Create a git repo with a remote at `dir` and one commit on `main`.
fn git_repo(dir: &Path, remote: &str) {
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git available");
        assert!(status.success(), "git {:?} failed", args);
    };
    std::fs::create_dir_all(dir).unwrap();
    git(&["init", "-b", "main"]);
    if !remote.is_empty() {
        git(&["remote", "add", "origin", remote]);
    }
    git(&[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "commit",
        "--allow-empty",
        "-m",
        "init",
    ]);
}

/// Run the whisper binary with a sandboxed environment.
fn turu(home: &Path, cwd: &Path) -> CliCommand {
    let mut cmd = CliCommand::cargo_bin("turu").unwrap();
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(cwd);
    cmd
}

#[test]
fn key_is_deterministic_and_canonical() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"cv/charly-vibes/whisper\""));

    // Same repo → same key, on repeat invocations.
    turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"cv/charly-vibes/whisper\""));
}

#[test]
fn https_remote_maps_to_host_key() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");

    turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"github.com/u/r\""));
}

#[test]
fn no_remote_falls_back_to_local_key() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("plain");
    git_repo(&repo, "");

    turu(tmp.path(), &repo)
        .args(["key", "--json"])
        .assert()
        .success()
        .stdout(contains("\"local/plain\""));
}

#[test]
fn resolve_branch_lands_in_the_checkout() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/branches/main/notes.md"));
}

// whisper-6fv — decision A of add-repo-private-scope: repo/branch knowledge
// lives in the checkout's own .whisper/ (travels with git); the machine-local
// store stays servable via recall composition and reachable via --global.

#[test]
fn repo_local_layout_matches_the_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/env.md"));

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/branches/main/notes.md"));
}

#[test]
fn unchanged_scopes_keep_their_store_destinations() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "global", "--json"])
        .assert()
        .success()
        .stdout(contains(".whisper/rules.md"));

    turu(tmp.path(), &repo)
        .args(["resolve", "worktree", "--json"])
        .assert()
        .success()
        .stdout(contains(
            ".whisper/repos/cv/charly-vibes/whisper/worktrees/.git/env.md",
        ));
}

#[test]
fn resolve_append_global_escape_hatch_writes_the_store() {
    let tmp = tempfile::tempdir().unwrap();
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
fn global_flag_rejected_outside_repo_and_branch_scopes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["append", "worktree", "--global", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--global applies only to the repo and branch scopes",
        ));

    turu(tmp.path(), &repo)
        .args(["append", "global", "--global", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--global applies only to the repo and branch scopes",
        ));
}

#[test]
fn recall_composes_store_major_with_dedup() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Repo-local entries (older) written in the checkout, then a NEWER
    // machine-local-store entry (the --global escape hatch). Store-major
    // composition serves every checkout entry before the newer store one.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "carol alpha"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args(["append", "repo", "--text", "bravo alpha"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-03T00:00:00Z")
        .args(["append", "repo", "--global", "--text", "alpha store"])
        .assert()
        .success();

    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["recall", "repo", "--json"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let a = out.find("carol alpha").unwrap();
    let b = out.find("bravo alpha").unwrap();
    let c = out.find("alpha store").unwrap();
    assert!(
        b < a && a < c,
        "store-major: checkout entries first (recency inside), then store entries; {out}"
    );
    // No id dedup false-positive: each entry appears exactly once.
    assert_eq!(out.matches("alpha").count(), 3);
}

#[test]
fn recall_serves_pre_existing_store_knowledge() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Knowledge written the pre-decision-A way, straight into the machine-
    // local store, must remain servable in the checkout (spec scenario).
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--global", "--text", "legacy store fact"])
        .assert()
        .success();

    turu(tmp.path(), &repo)
        .args(["recall", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("legacy store fact"));
}

#[test]
fn same_text_in_both_layers_stays_two_distinct_entries() {
    let tmp = tempfile::tempdir().unwrap();
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
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(out.matches("stated twice").count(), 2);
}

// whisper-eiy — tasks 2.1–2.2 of add-repo-private-scope: the private zone
// (`<checkout>/.whisper/private/`, gitignored) is a deterministic routing
// destination via `--private`, read-included in recall composition, and
// exact-path only (a `private/` nested elsewhere is just a name).

#[test]
fn private_flag_routes_into_the_zone_mirroring_the_layout() {
    let tmp = tempfile::tempdir().unwrap();
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
fn private_flag_rejected_like_the_escape_hatch() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["append", "worktree", "--private", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains(
            "--private applies only to the repo and branch scopes",
        ));

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
fn private_flag_requires_a_checkout() {
    let tmp = tempfile::tempdir().unwrap();
    // cwd is not a git repo: no repo-local root, no private zone.
    turu(tmp.path(), tmp.path())
        .args(["append", "repo", "--private", "--text", "x"])
        .assert()
        .failure()
        .stderr(contains("--private requires a git checkout"));
}

#[test]
fn recall_serves_the_private_zone_store_major() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Checkout layer: a private entry and a committed repo-local entry.
    // Store layer: a newer machine-local-store entry. Store-major order
    // serves every checkout entry (private + repo-local) before it.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch", "--private", "--text", "alpha private"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args(["append", "branch", "--text", "bravo committed"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-03T00:00:00Z")
        .args(["append", "branch", "--global", "--text", "charlie store"])
        .assert()
        .success();

    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["recall", "branch", "--json"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let a = out.find("alpha private").unwrap();
    let b = out.find("bravo committed").unwrap();
    let c = out.find("charlie store").unwrap();
    assert!(
        b < a && a < c,
        "checkout entries first (recency inside), then store entries; {out}"
    );
}

#[test]
fn private_and_committed_layers_keep_distinct_id_spaces() {
    let tmp = tempfile::tempdir().unwrap();
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
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(out.matches("stated twice").count(), 2);
}

#[test]
fn private_zone_is_exact_path_only() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // A `private/` directory nested deeper under .whisper/ is ordinary
    // content: recall never searches, so it is not served. Resolution is
    // exact-path (`.whisper/private/` only), never a name match.
    let nested = repo.join(".whisper/branches/main/private");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("notes.md"), "nested private line\n").unwrap();

    let out = turu(tmp.path(), &repo)
        .args(["recall", "branch", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    // Nothing servable in this scope — the nested dir is invisible, so the
    // recall is ok:true-empty (whisper-2c1 semantics), never an error.
    assert!(stdout.contains("\"empty\":true"), "{stdout}");
    assert!(!stdout.contains("nested private line"), "{stdout}");
}

// whisper-4xo — tasks 2.3–2.6 of add-repo-private-scope: transport and
// migration verbs can never carry or touch the private zone. "Detection
// that warns can be loose; anything that moves or publishes data cannot."

#[test]
fn pack_excludes_the_private_zone_and_reports_the_count() {
    let tmp = tempfile::tempdir().unwrap();
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
    // Exact-path rule: a privateX sibling is ordinary content — never
    // counted, never excluded as if it were the zone.
    let decoy = repo.join(".whisper/privateX/branches/main");
    std::fs::create_dir_all(&decoy).unwrap();
    std::fs::write(decoy.join("notes.md"), "- decoy entry\n").unwrap();

    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["bundle", "pack", "branch", "--json"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(out.contains("committed fact"));
    assert!(
        !out.contains("machine-secret"),
        "private text in bundle: {out}"
    );
    assert!(out.contains("\"private_excluded\":1"), "{out}");
}

#[test]
fn store_packed_bundle_unpacks_into_the_repo_local_root() {
    let tmp = tempfile::tempdir().unwrap();
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
    let landed = std::fs::read_to_string(repo.join(".whisper/env.md")).unwrap();
    assert!(landed.contains("legacy packed fact"), "{landed}");
}

#[test]
fn consolidate_leaves_the_checkout_private_zone_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Relocate the knowledge root into the checkout (repo-private
    // workspace_root override) — the closest consolidate can legally get
    // to checkout-local knowledge today.
    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!("workspace_root = '{}'\n", repo.join(".whisper").display()),
    )
    .unwrap();
    // A real legacy variant in the store, plus a decoy under the private
    // zone that looks exactly like one.
    let variant = repo.join(".whisper/repos/whisper");
    std::fs::create_dir_all(&variant).unwrap();
    std::fs::write(variant.join("env.md"), "legacy line\n").unwrap();
    let decoy = repo.join(".whisper/private/repos/whisper");
    std::fs::create_dir_all(&decoy).unwrap();
    std::fs::write(decoy.join("env.md"), "private line\n").unwrap();

    let out = String::from_utf8(
        turu(tmp.path(), &repo)
            .args(["consolidate", "--json"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(
        out.contains("repos/cv/charly-vibes/whisper"),
        "variant not migrated: {out}"
    );
    assert!(
        !variant.join("env.md").exists(),
        "legacy variant not consumed"
    );
    // The private zone is untouched: decoy still there, never reported.
    assert_eq!(
        std::fs::read_to_string(decoy.join("env.md")).unwrap(),
        "private line\n"
    );
    assert!(!out.contains("private/repos"), "{out}");
}

#[test]
fn init_provisions_the_escape_hatch_layer() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success();

    // Checkout layer ...
    assert!(repo.join(".whisper/env.md").exists());
    assert!(repo.join(".whisper/branches/main/notes.md").exists());
    // ... and the machine-local store layer (recall fallback + --global).
    assert!(
        tmp.path()
            .join(".whisper/repos/cv/charly-vibes/whisper/env.md")
            .exists()
    );
    assert!(
        tmp.path()
            .join(".whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md")
            .exists()
    );
}

#[test]
fn sync_managed_block_shows_repo_local_routing() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.contains("repo → `"));
    assert!(agents.contains(".whisper/env.md"));
    assert!(agents.contains("--global` escape hatch"));
    assert!(agents.contains("repos/cv/charly-vibes/whisper/env.md"));
    // whisper-eiy: the private zone shows in the routing map.
    assert!(agents.contains("--private` zone"));
    assert!(agents.contains(".whisper/private/env.md"));
    assert!(agents.contains(".whisper/private/branches/main/notes.md"));
}

// ---------------------------------------------------------------------------
// add-repo-private-scope 3.1 — checkout self-protection (ignore rule)
// ---------------------------------------------------------------------------

/// Assert the zone is effectively ignored via `git check-ignore` — the
/// same truth the implementation must use (text presence is not).
fn assert_effectively_ignored(repo: &Path, ignored: bool) {
    let out = Command::new("git")
        .args(["check-ignore", "-q", "--", ".whisper/private/probe"])
        .current_dir(repo)
        .status()
        .expect("git available");
    assert_eq!(
        out.code(),
        if ignored { Some(0) } else { Some(1) },
        "check-ignore .whisper/private: expected ignored={ignored}"
    );
}

#[test]
fn init_appends_private_ignore_rule_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    assert!(!repo.join(".gitignore").exists());

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":true"));

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".whisper/private/"), "{gitignore}");
    assert_effectively_ignored(&repo, true);
}

#[test]
fn init_ignore_rule_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    let first = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":false"));

    let second = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert_eq!(first, second, "re-init must not change .gitignore");
    assert_eq!(
        first.matches(".whisper/private/").count(),
        1,
        "exactly one rule"
    );
}

#[test]
fn init_preserves_existing_gitignore_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let original = "target/\n*.log\n";
    std::fs::write(repo.join(".gitignore"), original).unwrap();

    turu(tmp.path(), &repo).args(["init"]).assert().success();

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.starts_with(original), "{gitignore}");
    assert_eq!(gitignore.matches(".whisper/private/").count(), 1);
    assert_effectively_ignored(&repo, true);
}

#[test]
fn init_skips_rule_when_a_nested_gitignore_already_ignores_it() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    // Effective rule lives in a nested .gitignore — the checkout is already
    // self-protected; init must not append a redundant root rule (and must
    // not conjure a root .gitignore out of nothing).
    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(repo.join(".whisper/.gitignore"), "private/\n").unwrap();
    assert_effectively_ignored(&repo, true);

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":false"));

    assert!(!repo.join(".gitignore").exists());
}

#[test]
fn sync_ensures_the_private_ignore_rule_too() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();

    turu(tmp.path(), &repo)
        .args(["sync", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":true"));

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert_eq!(gitignore.matches(".whisper/private/").count(), 1);
    assert_effectively_ignored(&repo, true);
}

#[test]
fn ignore_rule_ensure_is_a_noop_outside_a_repo() {
    // No checkout → no private zone → nothing to protect. `init` must not
    // create a .gitignore in the cwd.
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();

    turu(tmp.path(), &plain)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":false"));
    assert!(!plain.join(".gitignore").exists());
}

// ---------------------------------------------------------------------------
// add-repo-private-scope 3.2 — doctor privacy integrity
// ---------------------------------------------------------------------------

#[test]
fn doctor_warns_when_private_zone_is_not_effectively_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-ignore"))
        .stdout(contains("not effectively ignored"));

    // Self-heal: init appends the rule exactly once, and the check passes.
    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    assert_eq!(
        std::fs::read_to_string(repo.join(".gitignore"))
            .unwrap()
            .matches(".whisper/private/")
            .count(),
        1
    );
    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-ignore"))
        .stdout(contains("\"warn\":0"));
}

#[test]
fn doctor_fails_prominently_on_tracked_files_under_private_zone() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    std::fs::create_dir_all(repo.join(".whisper/private")).unwrap();
    std::fs::write(
        repo.join(".whisper/private/env.md"),
        "secret machine facts\n",
    )
    .unwrap();
    let status = Command::new("git")
        .args(["add", "-f", ".whisper/private/env.md"])
        .current_dir(&repo)
        .status()
        .unwrap();
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
fn doctor_emits_leak_warnings_for_public_repo_local_files() {
    let tmp = tempfile::tempdir().unwrap();
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

    // Advisory: doctor still exits 0; the leak surfaces in warnings[].
    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-leaks"))
        .stdout(contains("machine path"))
        .stdout(contains("\"warn\":1"))
        .stdout(contains("\"fail\":0"));
}

#[test]
fn doctor_leak_lint_stays_quiet_on_clean_checkouts() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["init"]).assert().success();
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

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.private-leaks"))
        .stdout(contains("\"warn\":0"))
        .stdout(contains("\"fail\":0"));
}

#[test]
fn doctor_leak_lint_skips_superseded_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    let out = turu(tmp.path(), &repo)
        .args([
            "append",
            "repo",
            "--json",
            "--text",
            "deploy runs from /home/sasha/infra",
        ])
        .output()
        .unwrap();
    let envelope: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = envelope["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 64);

    // Supersede it with clean text — the dead entry must stop tripping the
    // advisory lint (superseded = dead by decision, cf. usage-staleness).
    turu(tmp.path(), &repo)
        .args([
            "append",
            "repo",
            "--json",
            "--supersedes",
            &id,
            "--text",
            "deploy needs vault login",
        ])
        .assert()
        .success();

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"warn\":0"))
        .stdout(contains("\"fail\":0"));
}

#[test]
fn append_creates_then_extends_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
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
    let content = std::fs::read_to_string(&env_md).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    for (line, text) in lines.iter().zip(["deploy needs vault", "second fact"]) {
        assert!(line.starts_with("- 20") && line.contains(" [id:"));
        assert!(line.ends_with(text));
    }
}

#[test]
fn init_never_overwrites() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success();
    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success();

    let rules = tmp.path().join(".whisper/rules.md");
    std::fs::write(&rules, "custom rules\n").unwrap();
    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), "custom rules\n");
}

#[test]
fn global_group_membership_routes_to_group_root() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.shared]\nroot = \"{}\"\nrepos = [\"cv/charly-vibes/whisper\"]\n",
            tmp.path().join("shared-knowledge").display()
        ),
    )
    .unwrap();

    turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .success()
        .stdout(contains(
            "shared-knowledge/repos/cv/charly-vibes/whisper/env.md",
        ));
}

#[test]
fn repo_private_config_joins_group() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.cv-tools]\nroot = \"{}\"\n",
            tmp.path().join("cv-root").display()
        ),
    )
    .unwrap();

    // Private, never-committed repo config.
    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(repo.join(".whisper/config.toml"), "group = \"cv-tools\"\n").unwrap();

    turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .success()
        .stdout(contains("cv-root/repos/cv/charly-vibes/whisper/env.md"));
}

#[test]
fn repo_private_root_override_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let config_dir = tmp.path().join(".config/whisper");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[groups.cv-tools]\nroot = \"{}\"\n",
            tmp.path().join("cv-root").display()
        ),
    )
    .unwrap();

    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!(
            "workspace_root = \"{}\"\n",
            tmp.path().join("private-ws").display()
        ),
    )
    .unwrap();

    // The override relocates the machine-local store: the global scope's
    // rules.md (and the repo/branch --global destinations) move there.
    turu(tmp.path(), &repo)
        .args(["resolve", "global", "--json"])
        .assert()
        .success()
        .stdout(contains("private-ws/rules.md"));

    // The checkout's repo-local root is unaffected by the override —
    // decision A routes repo/branch knowledge into the checkout itself.
    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("repo/.whisper/env.md"));
}

#[test]
fn group_scope_without_group_fails_with_suggestion() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "group", "--json"])
        .assert()
        .failure()
        .stderr(contains("no group is active"));
}

#[test]
fn sync_injects_managed_block_into_agents_md() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // First run creates the file; second run updates in place.
    turu(tmp.path(), &repo)
        .args(["sync", "--json"])
        .assert()
        .success()
        .stdout(contains("\"outcome\":\"created\""));

    turu(tmp.path(), &repo)
        .args(["sync", "--json"])
        .assert()
        .success()
        .stdout(contains("\"outcome\":\"updated\""));

    let agents = repo.join("AGENTS.md");
    let content = std::fs::read_to_string(&agents).unwrap();
    assert!(content.contains("<!-- TURU:START -->"));
    assert!(content.contains("cv/charly-vibes/whisper"));
    assert_eq!(content.matches("TURU:START").count(), 1);
}

#[test]
fn doctor_reports_unhealthy_then_healthy_after_sync() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.managed-block"))
        .stdout(contains("\"warn\":5"));

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"pass\":12"))
        .stdout(contains("\"warn\":0"));
}

#[test]
fn skill_install_writes_pack_and_doctor_reports_current() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["skill", "install", "--json"])
        .assert()
        .success()
        .stdout(contains("\"files\":11"));

    let router = repo.join(".turu/skills/whisper/whisper.md");
    assert!(router.is_file());
    assert!(repo.join(".turu/skills/whisper/subs/append.md").is_file());

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"turu.managed-skills\""))
        .stdout(contains("is current"));
}

#[test]
fn doctor_warns_when_repo_private_root_shadows_global_rules() {
    // GH-issue follow-up: a repo-private `workspace_root` intentionally beats
    // everything — which also relocates the global scope (`rules.md`) for
    // that repo. Doctor must surface the shadowing and any divergence.
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Global workspace with a rules file.
    let global_ws = tmp.path().join("global-ws");
    std::fs::create_dir_all(&global_ws).unwrap();
    std::fs::write(global_ws.join("rules.md"), "global rule\n").unwrap();
    std::fs::create_dir_all(home.join(".config/whisper")).unwrap();
    std::fs::write(
        home.join(".config/whisper/config.toml"),
        format!("workspace_root = '{}'\n", global_ws.display()),
    )
    .unwrap();

    // Repo-private root override shadows that global scope.
    let private_ws = tmp.path().join("private-ws");
    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!("workspace_root = '{}'\n", private_ws.display()),
    )
    .unwrap();

    // 1. Global rules.md exists but is hidden by the private root → warn.
    turu(&home, &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("hides"));

    // 2. Identical content in both → informational pass.
    std::fs::create_dir_all(&private_ws).unwrap();
    std::fs::write(private_ws.join("rules.md"), "global rule\n").unwrap();
    turu(&home, &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("in sync"));

    // 3. Diverged content → warn.
    std::fs::write(private_ws.join("rules.md"), "diverged\n").unwrap();
    turu(&home, &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("turu.shadowed-global"))
        .stdout(contains("diverged"));
}

#[test]
fn check_warns_when_repo_private_root_shadows_global_rules() {
    // whisper-122 follow-through: `turu check` is the fast-path validator —
    // it must surface the shadowing alongside `turu doctor`.
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    let global_ws = tmp.path().join("global-ws");
    std::fs::create_dir_all(&global_ws).unwrap();
    std::fs::write(global_ws.join("rules.md"), "global rule\n").unwrap();
    std::fs::create_dir_all(home.join(".config/whisper")).unwrap();
    std::fs::write(
        home.join(".config/whisper/config.toml"),
        format!("workspace_root = '{}'\n", global_ws.display()),
    )
    .unwrap();

    let private_ws = tmp.path().join("private-ws");
    std::fs::create_dir_all(repo.join(".whisper")).unwrap();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        format!("workspace_root = '{}'\n", private_ws.display()),
    )
    .unwrap();

    turu(&home, &repo)
        .args(["check", "--json"])
        .assert()
        .success()
        .stdout(contains("shadows the global rules file"));

    // In-sync copies do not warn.
    std::fs::create_dir_all(&private_ws).unwrap();
    std::fs::write(private_ws.join("rules.md"), "global rule\n").unwrap();
    turu(&home, &repo)
        .args(["check", "--json"])
        .assert()
        .success()
        .stdout(contains("workspace looks consistent"));
}

#[test]
fn feedback_dry_run_previews_body_and_fallback_url() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(&home, &repo)
        .args(["feedback", "bug", "--dry-run"])
        .write_stdin("steps to reproduce the bug\n")
        .assert()
        .success()
        .stdout(contains("fallback"))
        .stdout(contains("issues/new"))
        .stderr(contains("## Description"))
        .stderr(contains("steps to reproduce the bug"))
        .stderr(contains("Would file: gh issue create"));
}

#[test]
fn feedback_rejects_unknown_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(&home, &repo)
        .args(["feedback", "bugz"])
        .assert()
        .failure()
        .stdout(contains("unknown kind"))
        .stdout(contains("pass one of: bug, feature, question, chore"));
}

#[test]
fn feedback_requires_content() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = home.join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Closed stdin (not a terminal) and no --from-last-error: loud failure.
    turu(&home, &repo)
        .args(["feedback", "bug"])
        .assert()
        .failure()
        .stdout(contains("No issue content specified"));
}

#[test]
fn feedback_from_last_error_reads_the_scratch() {
    let tmp = tempfile::tempdir().unwrap();
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

#[test]
fn check_flags_missing_rules_file() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["check", "--json"])
        .assert()
        .success()
        .stdout(contains("rules file missing"));
}

// --- append ergonomics (whisper-hpd): positional text + stdin auto-detect ---

#[test]
fn append_accepts_positional_text() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // The form agents naturally write: text as a positional argument.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch", "positional fact"])
        .assert()
        .success()
        .stdout(contains("\"appended_bytes\""));

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).unwrap();
    assert!(notes.contains("positional fact"));
}

#[test]
fn append_reads_piped_stdin_without_stdin_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Piped stdin is auto-detected: no --stdin flag needed.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "branch"])
        .write_stdin("piped fact\n")
        .assert()
        .success();

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).unwrap();
    assert!(notes.contains("piped fact"));
}

#[test]
fn append_stdin_flag_before_scope_still_works() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    // Flag-order tolerance: --stdin before the positional scope.
    turu(tmp.path(), &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "--stdin", "branch"])
        .write_stdin("ordered fact\n")
        .assert()
        .success();

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).unwrap();
    assert!(notes.contains("ordered fact"));
}

#[test]
fn append_empty_piped_stdin_still_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["append", "branch"])
        .write_stdin("")
        .assert()
        .failure()
        .stderr(contains("nothing to append"));
}

#[test]
fn append_positional_text_wins_over_piped_stdin() {
    let tmp = tempfile::tempdir().unwrap();
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

    let notes = std::fs::read_to_string(repo.join(".whisper/branches/main/notes.md")).unwrap();
    assert!(notes.contains("explicit fact"));
    assert!(!notes.contains("ignored fact"));
}

// --- one-call recall loop (whisper-s8r): 'recall all' composes precedence ---

#[test]
fn recall_all_composes_global_repo_and_branch() {
    let tmp = tempfile::tempdir().unwrap();
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
        .unwrap();
    assert!(out.status.success());
    let json = String::from_utf8_lossy(&out.stdout);
    // Served via the envelope's entries: all three layers present.
    assert!(json.contains("global layer fact"));
    assert!(json.contains("repo layer fact"));
    assert!(json.contains("branch layer fact"));
}
