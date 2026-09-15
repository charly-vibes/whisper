//! End-to-end tests for the knowledge lifecycle changes: structured
//! entries (add-entry-model), recall serving, the distill contract, and
//! shared bundles.

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

fn repo_env(tmp: &tempfile::TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@github.com:u/r.git");
    (tmp.path().to_path_buf(), repo)
}

// ---------------------------------------------------------------------------
// add-entry-model
// ---------------------------------------------------------------------------

#[test]
fn append_writes_structured_entry_with_id() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "deploy fails on tuesday",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"duplicate\":false"))
        .stdout(contains("\"id\":\""));

    let out = turu(&home, &repo)
        .args(["resolve", "repo", "--json"])
        .output()
        .unwrap();
    let path = String::from_utf8(out.stdout)
        .unwrap()
        .split("\"path\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string();
    let content = std::fs::read_to_string(&path).unwrap();
    let line = content.lines().find(|l| l.starts_with("- ")).unwrap();
    assert!(line.starts_with("- 2026-01-01T00:00:00Z [id:"));
    assert!(line.ends_with("deploy fails on tuesday"));
}

#[test]
fn append_is_idempotent_within_the_same_second() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    for _ in 0..2 {
        turu(&home, &repo)
            .env("TURU_NOW", "2026-01-01T00:00:00Z")
            .args(["append", "repo", "--text", "same fact", "--json"])
            .assert()
            .success();
    }
    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content.lines().filter(|l| l.contains("same fact")).count(),
        1,
        "file must contain exactly one copy: {content}"
    );

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "same fact", "--json"])
        .assert()
        .stdout(contains("\"duplicate\":true"));
}

#[test]
fn restatement_later_is_a_new_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "stated twice"])
        .assert()
        .success();
    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args(["append", "repo", "--text", "stated twice"])
        .assert()
        .success();

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content
            .lines()
            .filter(|l| l.contains("stated twice") && l.contains("[id:"))
            .count(),
        2
    );
}

#[test]
fn topic_lands_in_the_entry_line() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "use kaniko", "--topic", "infra"])
        .assert()
        .success();

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains(" (#infra) use kaniko"));
}

#[test]
fn supersedes_marks_the_target_and_records_the_reference() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "old fact", "--json"])
        .assert()
        .success();
    let first_id = extract_id(&resolve_repo_path(&home, &repo), "old fact");

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "new fact",
            "--supersedes",
            &first_id,
            "--json",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(resolve_repo_path(&home, &repo)).unwrap();
    let old = content.lines().find(|l| l.contains("old fact")).unwrap();
    assert!(
        old.contains(&format!("(superseded by {first_id})")) || old.contains("(superseded by ")
    );
    let new_line = content.lines().find(|l| l.contains("new fact")).unwrap();
    assert!(new_line.contains(&format!("(supersedes {first_id})")));
}

#[test]
fn supersedes_unknown_id_fails_without_touching_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "kept fact"])
        .assert()
        .success();
    let before = std::fs::read_to_string(resolve_repo_path(&home, &repo)).unwrap();

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-02T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "new",
            "--supersedes",
            &"f".repeat(64),
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));

    let after = std::fs::read_to_string(resolve_repo_path(&home, &repo)).unwrap();
    assert_eq!(before, after);
}

#[test]
fn freeform_lines_are_preserved_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, repo) = repo_env(&tmp);

    let path = resolve_repo_path(&home, &repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "## notes\n\n- a plain bullet\n").unwrap();

    turu(&home, &repo)
        .env("TURU_NOW", "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "a fact"])
        .assert()
        .success();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("## notes"));
    assert!(content.contains("- a plain bullet"));
    assert!(content.contains("a fact"));
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn resolve_repo_path(home: &Path, repo: &Path) -> std::path::PathBuf {
    let out = turu(home, repo)
        .args(["resolve", "repo", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let raw = stdout.split("\"path\":\"").nth(1).unwrap();
    let raw = raw.split('"').next().unwrap();
    // The envelope escapes nothing on unix paths, but unescape just in case.
    std::path::PathBuf::from(raw.replace("\\\\", "\\"))
}

fn extract_id(path: &Path, needle: &str) -> String {
    let content = std::fs::read_to_string(path).unwrap();
    let line = content.lines().find(|l| l.contains(needle)).unwrap();
    let tok = line.split("[id:").nth(1).unwrap();
    tok.split(']').next().unwrap().to_string()
}
