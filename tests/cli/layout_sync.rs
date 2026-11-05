//! layout_sync integration tests.

use super::common::Uw;
use crate::common::{git_repo, turu};
use predicates::str::contains;
use std::process::Command;

use std::path::Path;
#[test]
fn init_appends_private_ignore_rule_when_absent() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    assert!(!repo.join(".gitignore").exists());

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":true"));

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).uw();
    assert!(gitignore.contains(".whisper/private/"), "{gitignore}");
    assert_effectively_ignored(&repo, true);
}
#[test]
fn init_ignore_rule_is_idempotent() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    let first = std::fs::read_to_string(repo.join(".gitignore")).uw();
    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":false"));

    let second = std::fs::read_to_string(repo.join(".gitignore")).uw();
    assert_eq!(first, second, "re-init must not change .gitignore");
    assert_eq!(
        first.matches(".whisper/private/").count(),
        1,
        "exactly one rule"
    );
}
#[test]
fn init_preserves_existing_gitignore_entries() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    let original = "target/\n*.log\n";
    std::fs::write(repo.join(".gitignore"), original).uw();

    turu(tmp.path(), &repo).args(["init"]).assert().success();

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).uw();
    assert!(gitignore.starts_with(original), "{gitignore}");
    assert_eq!(gitignore.matches(".whisper/private/").count(), 1);
    assert_effectively_ignored(&repo, true);
}
#[test]
fn init_skips_rule_when_a_nested_gitignore_already_ignores_it() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    // Effective rule lives in a nested .gitignore — the checkout is already
    // self-protected; init must not append a redundant root rule (and must
    // not conjure a root .gitignore out of nothing).
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(repo.join(".whisper/.gitignore"), "private/\n").uw();
    assert_effectively_ignored(&repo, true);

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":false"));

    assert!(!repo.join(".gitignore").exists());
}
#[test]
fn init_never_overwrites() {
    let tmp = tempfile::tempdir().uw();
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
    std::fs::write(&rules, "custom rules\n").uw();
    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(&rules).uw(), "custom rules\n");
}
#[test]
fn sync_ensures_the_private_ignore_rule_too() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    std::fs::write(repo.join(".gitignore"), "target/\n").uw();

    turu(tmp.path(), &repo)
        .args(["sync", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ignore_rule_added\":true"));

    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).uw();
    assert_eq!(gitignore.matches(".whisper/private/").count(), 1);
    assert_effectively_ignored(&repo, true);
}
#[test]
fn ignore_rule_ensure_is_a_noop_outside_a_repo() {
    // No checkout → no private zone → nothing to protect. `init` must not
    // create a .gitignore in the cwd.
    let tmp = tempfile::tempdir().uw();
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).uw();

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
fn sync_injects_managed_block_into_agents_md() {
    let tmp = tempfile::tempdir().uw();
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
    let content = std::fs::read_to_string(&agents).uw();
    assert!(content.contains("<!-- TURU:START -->"));
    assert!(content.contains("cv/charly-vibes/whisper"));
    assert_eq!(content.matches("TURU:START").count(), 1);
}
#[test]
fn re_sync_updates_only_the_block() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::write(repo.join("AGENTS.md"), "preamble kept intact\n").uw();

    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    let first = std::fs::read_to_string(repo.join("AGENTS.md")).uw();
    assert!(
        first.contains("preamble kept intact\n"),
        "content preserved"
    );

    turu(tmp.path(), &repo).args(["sync"]).assert().success();
    let second = std::fs::read_to_string(repo.join("AGENTS.md")).uw();
    // Outside the block, content is byte-identical; exactly one block.
    assert_eq!(second.matches("TURU:START").count(), 1);
    let strip = |s: &str| -> String {
        s.split("<!-- TURU:START -->")
            .next()
            .unwrap_or(s)
            .to_string()
    };
    assert_eq!(
        strip(&first),
        strip(&second),
        "content outside the block untouched"
    );
}
#[test]
fn sync_file_override_targets_the_named_file() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");

    turu(tmp.path(), &repo)
        .args(["sync", "--file", "CLAUDE.md", "--json"])
        .assert()
        .success()
        .stdout(contains("CLAUDE.md"))
        .stdout(contains("\"created\""));

    assert!(
        std::fs::read_to_string(repo.join("CLAUDE.md"))
            .uw()
            .contains("<!-- TURU:START -->")
    );
    assert!(!repo.join("AGENTS.md").exists(), "default target untouched");
}
#[test]
fn status_is_side_effect_free() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    let tree_snapshot = |dir: &Path| -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).uw().flatten() {
                let path = entry.path();
                if entry.file_type().uw().is_dir() {
                    stack.push(path);
                } else if entry.file_type().uw().is_file() {
                    // Regular files only: git may leave sockets (e.g.
                    // fsmonitor--daemon.ipc) that cannot be read.
                    let rel = path.strip_prefix(dir).uw().to_string_lossy().into_owned();
                    out.push((rel, std::fs::read(&path).uw()));
                }
            }
        }
        out.sort();
        out
    };

    let before = tree_snapshot(&repo);
    turu(tmp.path(), &repo)
        .args(["status", "--json"])
        .assert()
        .success();
    let after = tree_snapshot(&repo);
    assert_eq!(before, after, "status must not modify the workspace layout");
}
#[test]
fn skill_install_writes_pack_and_doctor_reports_current() {
    let tmp = tempfile::tempdir().uw();
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
fn init_under_machine_local_provisions_only_the_store_layer() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "routing = \"machine-local\"\n",
    )
    .uw();

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains(".whisper/repos/github.com/u/r/env.md"));

    assert!(
        tmp.path()
            .join(".whisper/repos/github.com/u/r/env.md")
            .exists(),
        "store layer provisioned"
    );
    assert!(
        !repo.join(".whisper/env.md").exists(),
        "no checkout repo slot under machine-local routing"
    );
    assert!(
        !repo.join(".whisper/branches/main/notes.md").exists(),
        "no checkout branch slot under machine-local routing"
    );
}

#[test]
fn init_provisions_the_escape_hatch_layer() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["init", "--json"])
        .assert()
        .success()
        .stdout(contains("\"created\""));

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
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).uw();
    assert!(agents.contains("repo → `"));
    assert!(agents.contains(".whisper/env.md"));
    assert!(agents.contains("--global` escape hatch"));
    assert!(agents.contains("repos/cv/charly-vibes/whisper/env.md"));
    // whisper-eiy: the private zone shows in the routing map.
    assert!(agents.contains("--private` zone"));
    assert!(agents.contains(".whisper/private/env.md"));
    assert!(agents.contains(".whisper/private/branches/main/notes.md"));
}

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
fn managed_block_paths_match_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "https://github.com/u/r.git");
    let synced = turu(tmp.path(), &repo).args(["sync"]).assert().success();
    assert_eq!(synced.get_output().status.code(), Some(0));
    let block = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();

    for (scope, marker) in [("repo", "repo → `"), ("branch", "branch → `")] {
        let start = block.find(marker).expect("block lists scope");
        let rest = &block[start + marker.len()..];
        let path = &rest[..rest.find('`').expect("path closed")];
        turu(tmp.path(), &repo)
            .args(["resolve", scope, "--json"])
            .assert()
            .success()
            .stdout(contains(path));
    }
}
