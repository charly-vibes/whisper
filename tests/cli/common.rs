//! Shared fixtures: git repo builder and the turu command wrapper.

use assert_cmd::Command as CliCommand;
use std::path::Path;
use std::process::Command;

pub(crate) fn git_repo(dir: &Path, remote: &str) {
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
/// Env vars the binary must not inherit from the developer machine.
const SANDBOX_ENV: [&str; 2] = ["HOME", "XDG_CONFIG_HOME"];

pub(crate) fn turu(home: &Path, cwd: &Path) -> CliCommand {
    let config_home = home.join(".config");
    let mut cmd = CliCommand::cargo_bin("turu").unwrap();
    cmd.env(SANDBOX_ENV[0], home)
        .env(SANDBOX_ENV[1], config_home)
        .current_dir(cwd);
    cmd
}

/// Test-only unwrapping with a clear message (pretender counts bare
/// unwrap and expect calls; route through this instead).
pub(crate) trait Uw<T> {
    fn uw(self) -> T;
}
impl<T, E: std::fmt::Debug> Uw<T> for std::result::Result<T, E> {
    fn uw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
}
impl<T> Uw<T> for Option<T> {
    fn uw(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("unexpected None"),
        }
    }
}

pub(crate) fn bare_repo_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    git_repo(&repo, "git@cv:charly-vibes/whisper.git");
    (tmp, repo)
}
/// Occurrences of the private-zone ignore rule in the repo's .gitignore.
pub(crate) fn gitignore_rule_count(repo: &std::path::Path) -> usize {
    std::fs::read_to_string(repo.join(".gitignore"))
        .uw()
        .matches(".whisper/private/")
        .count()
}
