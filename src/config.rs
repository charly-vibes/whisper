//! Layered configuration: global → group → repo.
//!
//! - **Global**: `~/.config/whisper/config.toml` — default workspace root,
//!   plus named `[groups.*]` that route a set of repos into one shared
//!   knowledge directory.
//! - **Repo**: `<repo>/.whisper/config.toml` — private (gitignored), never
//!   committed. Joins the repo to a group or fully overrides the root.
//!
//! Precedence: repo > group > global.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Result, WhisperError};

/// Global config at `~/.config/whisper/config.toml`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default workspace root (supports `~`). Defaults to `~/.whisper`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    /// Named groups: a set of repos routed into one shared workspace.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, GroupConfig>,
}

/// A group workspace shared by a set of repos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Directory where the group's knowledge tree lives (supports `~`).
    pub root: String,
    /// Explicit membership by canonical repo key (optional alternative
    /// to each repo joining privately via its own config).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<String>>,
}

/// Private per-repo config at `<repo>/.whisper/config.toml` (gitignored).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Join the group with this name (must exist in the global config).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Private workspace-root override for this checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
}

/// Fully resolved destinations after applying precedence.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Effective workspace root.
    pub workspace_root: PathBuf,
    /// Active group: (name, group root), if any.
    pub group: Option<(String, PathBuf)>,
}

impl Resolved {
    /// The root where repo/branch/worktree knowledge lands: the group root
    /// when a group is active, else the workspace root.
    pub fn knowledge_root(&self) -> &Path {
        match &self.group {
            Some((_, root)) => root,
            None => &self.workspace_root,
        }
    }
}

/// Path of the global config file (`$XDG_CONFIG_HOME/whisper/config.toml`).
pub fn global_path() -> PathBuf {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home_dir().join(".config"),
    };
    base.join("whisper").join("config.toml")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(path)
}

/// Walk up from `start` looking for `<dir>/.whisper/config.toml`.
/// Returns `(config dir, parsed config)` on the first hit.
pub fn find_repo_config(start: &Path) -> Option<(PathBuf, RepoConfig)> {
    let mut dir = Some(start.to_path_buf());
    while let Some(d) = dir {
        let candidate = d.join(".whisper").join("config.toml");
        if candidate.is_file() {
            let raw = std::fs::read_to_string(&candidate)
                .map_err(|e| format!("reading {}: {e}", candidate.display()))
                .ok()?;
            let cfg: RepoConfig = toml::from_str(&raw)
                .map_err(|e| format!("parsing {}: {e}", candidate.display()))
                .ok()?;
            return Some((d, cfg));
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

/// Load and resolve the layered configuration for the checkout containing
/// `cwd` (or the global default when `cwd` is not a repo).
pub fn load(cwd: &Path, repo_key: &str) -> Result<Resolved> {
    let global: GlobalConfig = match std::fs::read_to_string(global_path()) {
        Ok(raw) => toml::from_str(&raw)
            .map_err(|e| WhisperError::new(format!("parsing {}: {e}", global_path().display())))?,
        Err(_) => GlobalConfig::default(),
    };

    let default_root = expand_tilde(
        global
            .workspace_root
            .as_deref()
            .unwrap_or(DEFAULT_WORKSPACE_ROOT),
    );

    let repo_cfg = find_repo_config(cwd).map(|(_, cfg)| cfg);

    // Group resolution order: repo config's explicit `group`, then global
    // membership lists. Group root beats the global workspace root; a repo
    // private `workspace_root` beats everything.
    let mut group: Option<(String, PathBuf)> = None;
    if let Some(cfg) = &repo_cfg
        && let Some(name) = &cfg.group
    {
        let gc = global.groups.get(name).ok_or_else(|| {
            WhisperError::new(format!(
                "repo config joins group '{name}' but it is not defined in {}",
                global_path().display()
            ))
            .with_suggestion(format!(
                "add [groups.{name}] with a `root` to the global config, or remove `group = \"{name}\"` from .whisper/config.toml"
            ))
        })?;
        group = Some((name.clone(), expand_tilde(&gc.root)));
    }
    if group.is_none()
        && let Some((name, gc)) = global.groups.iter().find(|(_, g)| {
            g.repos
                .as_ref()
                .is_some_and(|r| r.iter().any(|k| k == repo_key))
        })
    {
        group = Some((name.clone(), expand_tilde(&gc.root)));
    }

    // Repo private root override.
    if let Some(cfg) = &repo_cfg
        && let Some(root) = &cfg.workspace_root
    {
        return Ok(Resolved {
            workspace_root: expand_tilde(root),
            group,
        });
    }

    Ok(Resolved {
        workspace_root: default_root,
        group,
    })
}

/// Default workspace root when nothing is configured.
pub const DEFAULT_WORKSPACE_ROOT: &str = "~/.whisper";

#[cfg(test)]
mod tests {
    use super::*;

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
        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        assert_eq!(cfg.groups["cv-tools"].root, "~/knowledge");
        assert_eq!(cfg.groups["cv-tools"].repos.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn repo_group_not_defined_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".whisper")).unwrap();
        std::fs::write(
            repo.join(".whisper/config.toml"),
            "group = \"missing-group\"\n",
        )
        .unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("xdg")) };
        let err = load(&repo, "github.com/u/r").unwrap_err();
        assert!(err.message.contains("not defined"));
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    }
}
