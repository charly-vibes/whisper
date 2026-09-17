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
fn resolve_branch_lands_under_workspace_root() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");

    turu(tmp.path(), &repo)
        .args(["resolve", "branch", "--json"])
        .assert()
        .success()
        .stdout(contains(
            ".whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md",
        ));
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

    let env_md = tmp
        .path()
        .join(".whisper/repos/cv/charly-vibes/whisper/env.md");
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

    turu(tmp.path(), &repo)
        .args(["resolve", "repo", "--json"])
        .assert()
        .success()
        .stdout(contains("private-ws/repos/cv/charly-vibes/whisper/env.md"));
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
        .stdout(contains("\"warn\":4"));

    turu(tmp.path(), &repo).args(["init"]).assert().success();
    turu(tmp.path(), &repo).args(["sync"]).assert().success();

    turu(tmp.path(), &repo)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"pass\":8"))
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
