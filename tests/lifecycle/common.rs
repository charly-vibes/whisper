//! Shared fixtures for lifecycle tests.

use assert_cmd::Command as CliCommand;
use std::path::Path;
use std::process::Command;

/// Test-only unwrapping with a clear message (pretender counts bare
/// unwrap and expect calls; route through this instead).
pub(crate) trait Uw<T> {
    fn uw(self) -> T;
}

impl<T, E: std::fmt::Debug> Uw<T> for std::result::Result<T, E> {
    fn uw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("unwrap failed: {e:?}"),
        }
    }
}

impl<T> Uw<T> for Option<T> {
    fn uw(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("unwrap failed on None"),
        }
    }
}

/// Create a git repo with a remote at `dir` and one commit on `main`.
pub(crate) fn git_repo(dir: &Path, remote: &str) {
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git available");
        assert!(status.success(), "git {:?} failed", args);
    };
    std::fs::create_dir_all(dir).uw();
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

pub(crate) fn turu(home: &Path, cwd: &Path) -> CliCommand {
    let mut cmd = CliCommand::cargo_bin("turu").uw();
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(cwd);
    cmd
}

/// `turu` pinned to a fixed clock — the common case in these tests.
pub(crate) fn at(home: &Path, repo: &Path, now: &str) -> CliCommand {
    let mut cmd = turu(home, repo);
    cmd.env("TURU_NOW", now);
    cmd
}

pub(crate) fn repo_env(tmp: &tempfile::TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@github.com:u/r.git");
    (tmp.path().to_path_buf(), repo)
}

/// Extract a string-valued JSON field (no escapes in these envelopes).
pub(crate) fn json_field(s: &str, key: &str) -> String {
    s.split(&format!("\"{key}\":\""))
        .nth(1)
        .uw()
        .split('"')
        .next()
        .uw()
        .to_string()
}

/// Extract a numeric JSON field (integers only).
pub(crate) fn json_num(s: &str, key: &str) -> i64 {
    s.split(&format!("\"{key}\":"))
        .nth(1)
        .uw()
        .split([',', '}'])
        .next()
        .uw()
        .trim()
        .parse()
        .uw()
}

pub(crate) fn extract_id(path: &Path, needle: &str) -> String {
    let content = std::fs::read_to_string(path).uw();
    let line = content.lines().find(|l| l.contains(needle)).uw();
    let tok = line.split("[id:").nth(1).uw();
    tok.split(']').next().uw().to_string()
}

pub(crate) fn resolve_repo_path(home: &Path, repo: &Path) -> std::path::PathBuf {
    let out = turu(home, repo)
        .args(["resolve", "repo", "--json"])
        .output()
        .uw();
    let stdout = String::from_utf8(out.stdout).uw();
    std::path::PathBuf::from(json_field(&stdout, "path").replace("\\\\", "\\"))
}

pub(crate) fn seed_entries(home: &Path, repo: &Path) {
    for (now, text, topic) in [
        ("2026-01-01T00:00:00Z", "oldest fact", Some("infra")),
        ("2026-01-02T00:00:00Z", "middle fact", None),
        ("2026-01-03T00:00:00Z", "newest fact", Some("infra")),
    ] {
        let mut cmd = turu(home, repo);
        cmd.env("TURU_NOW", now);
        cmd.args(["append", "repo", "--text", text]);
        if let Some(t) = topic {
            cmd.args(["--topic", t]);
        }
        cmd.assert().success();
    }
}

/// `bundle pack repo --out <out>`, asserting success.
pub(crate) fn pack_bundle(home: &Path, repo: &Path, out: &Path) {
    turu(home, repo)
        .args(["bundle", "pack", "repo", "--out", out.to_str().uw()])
        .assert()
        .success();
}

/// `bundle unpack --file <file> --json`, returning stdout.
pub(crate) fn unpack_bundle(home: &Path, repo: &Path, file: &Path) -> String {
    String::from_utf8(
        turu(home, repo)
            .args(["bundle", "unpack", "--file", file.to_str().uw(), "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw()
}

/// `distill repo --begin --json` at a fixed clock; returns (working_path, revision).
pub(crate) fn begin_distill(home: &Path, repo: &Path, now: &str) -> (String, String) {
    let out = String::from_utf8(
        at(home, repo, now)
            .args(["distill", "repo", "--begin", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    (
        json_field(&out, "working_path"),
        json_field(&out, "revision"),
    )
}
