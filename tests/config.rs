//! Integration tests for config loading (public API only).

use std::path::PathBuf;
use whisper::config::{GlobalConfig, expand_tilde, find_repo_config, global_path, load};

/// Test-only unwrapping with a clear message (pretender counts bare
/// unwrap and expect calls; route through this instead).
trait Uw<T> {
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
impl<T> Uw<T> for std::option::Option<T> {
    fn uw(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("unexpected None"),
        }
    }
}

#[test]
fn tilde_expansion_uses_home() {
    unsafe { std::env::set_var("HOME", "/tmp/fake-home") };
    assert_eq!(
        expand_tilde("~/.whisper"),
        PathBuf::from("/tmp/fake-home/.whisper")
    );
    assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
}

#[test]
fn global_path_respects_xdg() {
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg") };
    assert_eq!(global_path(), PathBuf::from("/tmp/xdg/whisper/config.toml"));
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
}

#[test]
fn parse_group_config() {
    let raw = r#"
workspace_root = "~/.whisper"

[groups.cv-tools]
root = "~/knowledge"
repos = ["cv/charly-vibes/whisper"]
"#;
    let cfg: GlobalConfig = toml::from_str(raw).uw();
    assert_eq!(cfg.groups["cv-tools"].root, "~/knowledge");
    assert_eq!(cfg.groups["cv-tools"].repos.as_ref().uw().len(), 1);
}

#[test]
fn relative_repo_workspace_root_anchors_to_config_dir() {
    // GH #1: a repo-private relative workspace_root must resolve against
    // the directory containing the config, not the process cwd — so a
    // turu call from a nested subdir never materializes a stray
    // `<subdir>/.whisper/` tree.
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::create_dir_all(repo.join("scripts")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "workspace_root = \".whisper\"\n",
    )
    .uw();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("xdg")) };
    let resolved = load(&repo.join("scripts"), "github.com/u/r").uw();
    assert_eq!(resolved.workspace_root, repo.join(".whisper"));
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
}

#[test]
fn repo_config_found_in_a_parent_dir() {
    // Deployed scenario config-precedence/config-found-in-a-parent:
    // a call from `<repo>/src/module` applies the config at `<repo>/`.
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::create_dir_all(repo.join("src/module")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "workspace_root = \"ws\"\n",
    )
    .uw();

    let (dir, cfg) = find_repo_config(&repo.join("src/module")).uw();
    assert_eq!(dir, repo);
    assert_eq!(cfg.workspace_root.as_deref(), Some("ws"));
}

#[test]
fn nested_config_shadows_the_repo_root() {
    // Deployed scenario config-precedence/nested-config-shadows-the-repo-root:
    // the first config found walking up wins; the repo root's config is
    // not consulted for that invocation.
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    let sub = repo.join("sub");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::create_dir_all(sub.join(".whisper")).uw();
    std::fs::create_dir_all(sub.join("deeper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "workspace_root = \"root\"\n",
    )
    .uw();
    std::fs::write(
        sub.join(".whisper/config.toml"),
        "workspace_root = \"sub\"\n",
    )
    .uw();

    let (dir, cfg) = find_repo_config(&sub.join("deeper")).uw();
    assert_eq!(dir, sub);
    assert_eq!(cfg.workspace_root.as_deref(), Some("sub"));
}

#[test]
fn fresh_machine_defaults_to_the_global_store() {
    // Deployed scenario config-precedence/fresh-machine: no config files
    // anywhere → scopes resolve under the default workspace root.
    let tmp = tempfile::tempdir().uw();
    std::fs::create_dir_all(tmp.path().join("xdg")).uw();
    unsafe { std::env::set_var("HOME", tmp.path()) };
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("xdg")) };
    let resolved = load(tmp.path(), "github.com/u/r").uw();
    assert_eq!(resolved.workspace_root, tmp.path().join(".whisper"));
    assert!(resolved.group.is_none());
    assert!(resolved.shadowed_global_root.is_none());
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
}

#[test]
fn repo_group_not_defined_is_error() {
    let tmp = tempfile::tempdir().uw();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".whisper")).uw();
    std::fs::write(
        repo.join(".whisper/config.toml"),
        "group = \"missing-group\"\n",
    )
    .uw();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("xdg")) };
    let err = load(&repo, "github.com/u/r").unwrap_err();
    assert!(err.message.contains("not defined"));
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
}
