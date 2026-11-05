//! Scope enum, write targets, and the append ledger operation.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Resolved;
use crate::entry::{self, Entry, Item};
use crate::{Result, WhisperError};

use super::Facts;

/// Knowledge routing scope (from the incitaciones whisper skill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// True for every repo and project: `rules.md`.
    Global,
    /// Repo-wide infra facts: `repos/<key>/env.md`.
    Repo,
    /// Branch-specific notes: `repos/<key>/branches/<slug>/notes.md`.
    Branch,
    /// Worktree-local setup: `repos/<key>/worktrees/<slot>/env.md`.
    Worktree,
    /// Shared repo-env knowledge in the active group's workspace.
    Group,
}

impl std::str::FromStr for Scope {
    type Err = WhisperError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "global" => Ok(Scope::Global),
            "repo" => Ok(Scope::Repo),
            "branch" => Ok(Scope::Branch),
            "worktree" => Ok(Scope::Worktree),
            "group" => Ok(Scope::Group),
            other => Err(WhisperError::new(format!("unknown scope '{other}'"))
                .with_suggestion("valid scopes: global | repo | branch | worktree | group")),
        }
    }
}

/// A resolved write destination.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub scope: Scope,
    pub path: PathBuf,
}

impl Target {
    /// Create the file and its parents if missing; return whether it was created.
    pub fn ensure(&self) -> std::io::Result<bool> {
        if self.path.exists() {
            return Ok(false);
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(&self.path)?;
        Ok(true)
    }

    /// Append text verbatim, normalizing to a single trailing newline.
    pub fn append(&self, text: &str) -> std::io::Result<()> {
        self.ensure()?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&self.path)?;
        let trimmed = text.trim_end();
        writeln!(f, "{trimmed}")?;
        Ok(())
    }
}

/// Deterministic scope key used in entry-id hashing, so identical text in
/// different scopes (or branches) never collides.
pub fn scope_key(scope: Scope, facts: &Facts, resolved: &Resolved) -> String {
    match scope {
        Scope::Global => "global".to_string(),
        Scope::Repo => facts.repo_key.clone(),
        Scope::Branch => format!("{}/{}", facts.repo_key, facts.branch_slug),
        Scope::Worktree => format!("{}/{}", facts.repo_key, facts.worktree_slot),
        Scope::Group => match &resolved.group {
            Some((name, _)) => format!("group:{name}/{}", facts.repo_key),
            None => facts.repo_key.clone(), // unreachable: resolve() errors first
        },
    }
}

/// Result of an entry append.
#[derive(Debug, Serialize)]
pub struct AppendReport {
    pub id: String,
    pub duplicate: bool,
    /// Id of the entry that was marked superseded (when `supersedes` was given).
    pub superseded: Option<String>,
    /// Bytes actually written (the rendered entry line, not the raw input).
    pub bytes: usize,
}

/// Append one structured entry to a scope file: idempotent by id, with
/// mechanical supersede marking. Unmanaged freeform lines are preserved
/// verbatim in their original order.
pub fn append_entry(
    target: &Target,
    scope_key: &str,
    text: &str,
    topic: Option<&str>,
    supersedes: Option<&str>,
    ts: &str,
) -> Result<AppendReport> {
    validate_topic(topic)?;
    let text = normalize_text(text);
    let id = entry::entry_id(scope_key, ts, &text);
    let raw = std::fs::read_to_string(&target.path).unwrap_or_default();
    let duplicate = already_present(&raw, &id);
    let new = Entry {
        ts: ts.to_string(),
        id: id.clone(),
        topic: topic.map(str::to_string),
        text: text.to_string(),
        supersedes: None,
        superseded_by: None,
    };
    if duplicate {
        return Ok(duplicate_report(id));
    }
    if let Some(supersedes) = supersedes {
        return rewrite_with_supersede(target, &raw, new, id, supersedes);
    }
    fast_path_append(target, &raw, new, id)
}

/// Duplicate writes write nothing — id preserved for reference.
fn duplicate_report(id: String) -> AppendReport {
    AppendReport {
        id,
        duplicate: true,
        superseded: None,
        bytes: 0,
    }
}

/// Topics are bare keys: no whitespace, parens, or `#`.
fn validate_topic(topic: Option<&str>) -> Result<()> {
    if let Some(t) = topic {
        let invalid = t.is_empty()
            || t.chars()
                .any(|c| c.is_whitespace() || c == '(' || c == ')' || c == '#');
        if invalid {
            return Err(
                WhisperError::new(format!("invalid topic '{t}'")).with_suggestion(
                    "topics are bare keys like 'infra' — no whitespace, parens, or '#'",
                ),
            );
        }
    }
    Ok(())
}

/// Normalize: no trailing whitespace overall, and no line may start with
/// whitespace — continuation lines start with exactly two spaces, so
/// leading whitespace would break the ledger round-trip.
fn normalize_text(text: &str) -> String {
    text.trim_end()
        .lines()
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Duplicate guard: an entry with the same id is already in the file.
fn already_present(raw: &str, id: &str) -> bool {
    entry::parse_file(raw)
        .iter()
        .any(|i| matches!(i, Item::Entry(e) if e.id == id))
}

/// Supersede path: mark the target entry superseded and rewrite the file.
#[allow(clippy::too_many_arguments)]
fn rewrite_with_supersede(
    target: &Target,
    raw: &str,
    mut new: Entry,
    id: String,
    sup: &str,
) -> Result<AppendReport> {
    let mut items = entry::parse_file(raw);
    mark_superseded(&mut items, sup, &id)?;
    new.supersedes = Some(sup.to_string());
    items.push(Item::Entry(new));
    target.ensure().map_err(WhisperError::from)?;
    // Rewrite path (rare): concurrent appends during this window can be
    // lost — see design.md; the fast path below is the safe default.
    let rendered = entry::render_file(&items);
    std::fs::write(&target.path, &rendered).map_err(WhisperError::from)?;
    Ok(AppendReport {
        id,
        duplicate: false,
        superseded: Some(sup.to_string()),
        bytes: rendered.len(),
    })
}

/// Fast path: single O_APPEND write — concurrent agents appending in the
/// same window cannot lose each other's entries.
fn fast_path_append(target: &Target, raw: &str, new: Entry, id: String) -> Result<AppendReport> {
    target.ensure().map_err(WhisperError::from)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&target.path)
        .map_err(WhisperError::from)?;
    let rendered = entry::render_file(&[Item::Entry(new)]);
    let line = if !raw.is_empty() && !raw.ends_with('\n') {
        format!("\n{rendered}")
    } else {
        rendered
    };
    f.write_all(line.as_bytes()).map_err(WhisperError::from)?;
    Ok(AppendReport {
        id,
        duplicate: false,
        superseded: None,
        bytes: line.len(),
    })
}

/// Mark the supersedes-target entry superseded (error when it is not in
/// this scope file).
fn mark_superseded(items: &mut [Item], sup: &str, id: &str) -> Result<()> {
    match items
        .iter_mut()
        .find(|i| matches!(i, Item::Entry(e) if e.id == sup))
    {
        Some(Item::Entry(e)) => {
            e.superseded_by = Some(id.to_string());
            Ok(())
        }
        _ => Err(
            WhisperError::new(format!("supersedes target not found in this scope: {sup}"))
                .with_suggestion("turu recall <scope> --include-superseded to list entry ids"),
        ),
    }
}
