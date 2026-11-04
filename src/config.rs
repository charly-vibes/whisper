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
    /// Declarative routing mode for the repo/branch scopes (gh-3):
    /// `repo-local` (decision-A default — knowledge travels with the
    /// checkout's git) or `machine-local` (the persistent equivalent of
    /// the `--global` escape hatch — knowledge lands in the machine-local
    /// store and the checkout stays clean).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<String>,
}

/// Declarative routing mode for the repo/branch scopes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Routing {
    /// Repo/branch knowledge lands in the checkout's committed `.whisper/`
    /// zone (decision A default).
    #[default]
    RepoLocal,
    /// Repo/branch knowledge lands in the machine-local store — the
    /// persistent `--global` default (gh-3: no per-invocation flag needed).
    MachineLocal,
}

impl Routing {
    /// Parse a routing-mode config value, erroring with the valid values
    /// on anything else.
    pub fn parse_value(raw: &str) -> Result<Routing> {
        match raw {
            "repo-local" => Ok(Routing::RepoLocal),
            "machine-local" => Ok(Routing::MachineLocal),
            other => Err(WhisperError::new(format!(
                "unknown routing mode '{other}' in .whisper/config.toml"
            ))
            .with_suggestion(
                "set routing to 'repo-local' (checkout-committed, default) or 'machine-local' (machine-local store)",
            )),
        }
    }
}

/// Fully resolved destinations after applying precedence.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Effective workspace root.
    pub workspace_root: PathBuf,
    /// Active group: (name, group root), if any.
    pub group: Option<(String, PathBuf)>,
    /// The workspace root this checkout would use without the repo-private
    /// override — present exactly when a repo config sets its own
    /// `workspace_root`. That override also relocates the global scope
    /// (`rules.md`), so doctor compares it against the shadowed location.
    pub shadowed_global_root: Option<PathBuf>,
    /// Declarative routing mode for the repo/branch scopes (gh-3).
    pub routing: Routing,
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
    let default_root = global_default_root()?;
    let (repo_cfg_dir, repo_cfg) = find_repo_config(cwd)
        .map(|(dir, cfg)| (Some(dir), Some(cfg)))
        .unwrap_or((None, None));
    let routing = routing_of(&repo_cfg)?;
    let group = resolve_group(&repo_cfg, &load_global()?, repo_key)?;

    // Repo private root override (anchored to the repo config dir — see
    // `private_root_override`) beats the workspace root for the global scope.
    if let Some(resolved) = private_root_override(
        &repo_cfg_dir,
        &repo_cfg,
        group.clone(),
        routing,
        &default_root,
    ) {
        return Ok(resolved);
    }

    Ok(Resolved {
        workspace_root: default_root,
        group,
        shadowed_global_root: None,
        routing,
    })
}

/// Parse the global config (default when the file is absent).
fn load_global() -> Result<GlobalConfig> {
    match std::fs::read_to_string(global_path()) {
        Ok(raw) => toml::from_str(&raw)
            .map_err(|e| WhisperError::new(format!("parsing {}: {e}", global_path().display()))),
        Err(_) => Ok(GlobalConfig::default()),
    }
}

/// The global workspace root (explicit or built-in default), tilde-expanded.
fn global_default_root() -> Result<PathBuf> {
    let global = load_global()?;
    Ok(expand_tilde(
        global
            .workspace_root
            .as_deref()
            .unwrap_or(DEFAULT_WORKSPACE_ROOT),
    ))
}

/// Declarative routing mode (gh-3): validated eagerly so a typo in
/// `.whisper/config.toml` fails the invocation with a suggestion
/// instead of silently falling back to the default mode.
fn routing_of(repo_cfg: &Option<RepoConfig>) -> Result<Routing> {
    match repo_cfg.as_ref().and_then(|c| c.routing.as_ref()) {
        Some(raw) => Routing::parse_value(raw),
        None => Ok(Routing::RepoLocal),
    }
}

/// Group resolution order: repo config's explicit `group`, then global
/// membership lists. Group root beats the global workspace root for
/// repo/branch/worktree scopes.
fn resolve_group(
    repo_cfg: &Option<RepoConfig>,
    global: &GlobalConfig,
    repo_key: &str,
) -> Result<Option<(String, PathBuf)>> {
    if let Some(cfg) = repo_cfg
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
        return Ok(Some((name.clone(), expand_tilde(&gc.root))));
    }
    Ok(global
        .groups
        .iter()
        .find(|(_, g)| {
            g.repos
                .as_ref()
                .is_some_and(|r| r.iter().any(|k| k == repo_key))
        })
        .map(|(name, gc)| (name.clone(), expand_tilde(&gc.root))))
}

/// Repo private `workspace_root` override. A relative path is anchored to
/// the directory containing the repo config, so resolution never depends on
/// the process cwd (GH #1: subdirectory calls must not spawn stray
/// workspace trees).
#[allow(clippy::too_many_arguments)]
fn private_root_override(
    repo_cfg_dir: &Option<PathBuf>,
    repo_cfg: &Option<RepoConfig>,
    group: Option<(String, PathBuf)>,
    routing: Routing,
    default_root: &Path,
) -> Option<Resolved> {
    let (cfg_dir, cfg) = (repo_cfg_dir.as_ref()?, repo_cfg.as_ref()?);
    let root = cfg.workspace_root.as_ref()?;
    let expanded = expand_tilde(root);
    let anchored = if expanded.is_absolute() {
        expanded
    } else {
        cfg_dir.join(expanded)
    };
    Some(Resolved {
        workspace_root: anchored,
        group,
        shadowed_global_root: Some(default_root.to_path_buf()),
        routing,
    })
}

/// Default workspace root when nothing is configured.
pub const DEFAULT_WORKSPACE_ROOT: &str = "~/.whisper";
