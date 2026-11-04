//! Managed-block generation and injection into AGENTS.md.

use std::path::{Path, PathBuf};

use genesis::managed_block::{BlockDef, BlockInjector, BlockRegistry, InjectResult};

use crate::config::Resolved;
use crate::{Result, WhisperError};

use super::{BLOCK_NAME, Facts, Scope, agents_file, private_zone_target, resolve};

pub fn turu_injector() -> BlockInjector {
    let mut registry = BlockRegistry::new();
    registry.register(BlockDef::with_markers(
        BLOCK_NAME,
        "<!-- TURU:START -->\n",
        "\n<!-- TURU:END -->\n",
    ));
    BlockInjector::new(registry)
}

/// Content of the turu managed block for the current checkout: the
/// deterministic routing map, so agents read paths from a file instead of
/// re-deriving them.
pub fn managed_block_content(facts: &Facts, resolved: &Resolved) -> Result<String> {
    let path_of = |scope| resolve(scope, facts, resolved).map(|t| t.path);
    let fmt = |p: Result<PathBuf>| p.map(|p| format!("`{}`", p.display()));
    let store_layer = store_layer(facts, resolved);
    Ok(format!(
        "# Whisper knowledge workspace (managed by turu — regenerate with `turu sync`)\n\
         \n\
         - Workspace root: `{}`{}\n\
         - Repo key: `{}` · Branch slug: `{}` · Worktree slot: `{}`\n\
         - Deterministic routing (resolve, never guess):\n\
           - global → {}\n\
           - repo → {}\n\
           - branch → {}\n\
           - worktree → {}\n\
         {}\
         - Commands: `turu resolve <scope>` · `turu append <scope> [--global] --text ... [--topic k] [--supersedes id]` · `turu recall <scope> [--topic k] [--budget bytes]` · `turu distill <scope> --begin|--commit` · `turu bundle pack|unpack` · `turu status` · `turu doctor` · `turu feedback <kind>`\n",
        resolved.workspace_root.display(),
        group_note(resolved),
        facts.repo_key,
        facts.branch_slug,
        facts.worktree_slot,
        fmt(path_of(Scope::Global)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Repo)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Branch)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Worktree)).unwrap_or_else(|_| "(unavailable)".into()),
        store_layer,
    ))
}

/// `(group \`n\` active — shared scopes route there)` when a group is active.
fn group_note(resolved: &Resolved) -> String {
    resolved
        .group
        .as_ref()
        .map(|(n, _)| format!(" (group `{n}` active — shared scopes route there)"))
        .unwrap_or_default()
}

/// The checkout-owned layer lines (checkout, store, private destinations);
/// empty outside a checkout (the store is then the only layer).
fn store_layer(facts: &Facts, resolved: &Resolved) -> String {
    if facts.repo_local_root.is_none() {
        return String::new();
    }
    let path_of = |scope| resolve(scope, facts, resolved).map(|t| t.path);
    let fmt = |p: Result<PathBuf>| p.map(|p| format!("`{}`", p.display()));
    let store_of = |scope| {
        let f = Facts {
            repo_local_root: None,
            ..facts.clone()
        };
        resolve(scope, &f, resolved).map(|t| t.path)
    };
    let private_of = |scope| private_zone_target(scope, facts, resolved).map(|t| t.path);
    format!(
        "  - append/resolve default: repo → {}, branch → {}\n  - `--global` escape hatch: repo → {}, branch → {}\n  - `--private` zone (never pushed): repo → {}, branch → {}\n",
        fmt(path_of(Scope::Repo)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(path_of(Scope::Branch)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(store_of(Scope::Repo)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(store_of(Scope::Branch)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(private_of(Scope::Repo)).unwrap_or_else(|_| "(unavailable)".into()),
        fmt(private_of(Scope::Branch)).unwrap_or_else(|_| "(unavailable)".into()),
    )
}

/// Inject/refresh the turu managed block in the agent-facing file.
/// Returns the target path and one of `created | prepended | updated`.
pub fn agents_sync(
    repo_root: &Path,
    target_override: Option<&str>,
    facts: &Facts,
    resolved: &Resolved,
) -> Result<(PathBuf, &'static str)> {
    let path = match target_override {
        Some(f) => PathBuf::from(f),
        None => agents_file(repo_root),
    };
    let content = managed_block_content(facts, resolved)?;
    let result = turu_injector()
        .inject(&path, BLOCK_NAME, &content)
        .map_err(WhisperError::from)?;
    let label = match result {
        InjectResult::Created => "created",
        InjectResult::Prepended => "prepended",
        InjectResult::Updated => "updated",
    };
    Ok((path, label))
}
