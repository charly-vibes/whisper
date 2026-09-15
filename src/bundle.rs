//! Shared-store primitive: deterministic, transport-agnostic knowledge
//! bundles keyed by the canonical repo key. Packing identical state yields
//! identical bytes; unpack merges extend-without-duplicate (the
//! `consolidate` rules) and never overwrites existing content.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::Resolved;
use crate::entry::{self, Entry, Item};
use crate::workspace::{self, Facts, Scope};
use crate::{Result, WhisperError};

/// A knowledge bundle: content + addressing only, no transport state.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub repo_key: String,
    pub scope: String,
    /// Entries sorted by id — deterministic byte order.
    pub entries: Vec<Entry>,
    /// Unmanaged freeform lines, in file order.
    pub freeform: Vec<String>,
}

/// `turu bundle pack <scope>`: emit the bundle (to `--out` as a file, or in
/// the envelope). Packing identical state twice yields identical bytes.
pub fn pack(
    scope: Scope,
    facts: &Facts,
    resolved: &Resolved,
    out: Option<&str>,
) -> Result<serde_json::Value> {
    // resolve() inherits the group-scope error when no group is active.
    let target = workspace::resolve(scope, facts, resolved)?;
    let raw = std::fs::read_to_string(&target.path).unwrap_or_default();
    let mut entries: Vec<Entry> = Vec::new();
    let mut freeform: Vec<String> = Vec::new();
    for item in entry::parse_file(&raw) {
        match item {
            Item::Entry(e) => entries.push(e),
            Item::Line(l) if !l.trim().is_empty() => freeform.push(l),
            Item::Line(_) => {}
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let bundle = Bundle {
        repo_key: facts.repo_key.clone(),
        scope: scope_name(scope).to_string(),
        entries,
        freeform,
    };
    match out {
        Some(path) => {
            let bytes = serde_json::to_string_pretty(&bundle)
                .map_err(|e| WhisperError::new(format!("serializing bundle: {e}")))?;
            std::fs::write(path, &bytes).map_err(WhisperError::from)?;
            Ok(json!({
                "path": path,
                "sha256": entry::sha256_hex(&bytes),
                "repo_key": bundle.repo_key,
                "scope": bundle.scope,
                "entries": bundle.entries.len(),
                "freeform_lines": bundle.freeform.len(),
            }))
        }
        None => {
            let value = serde_json::to_value(&bundle)
                .map_err(|e| WhisperError::new(format!("serializing bundle: {e}")))?;
            Ok(json!({
                "bundle": value,
                "repo_key": bundle.repo_key,
                "scope": bundle.scope,
                "entries": bundle.entries.len(),
                "freeform_lines": bundle.freeform.len(),
            }))
        }
    }
}

/// `turu bundle unpack`: merge a bundle into the local workspace using
/// extend-without-duplicate rules; entry ids already present are skipped
/// and reported; existing file content is never overwritten.
pub fn unpack(input: &str, facts: &Facts, resolved: &Resolved) -> Result<serde_json::Value> {
    let bundle: Bundle = serde_json::from_str(input)
        .map_err(|e| WhisperError::new(format!("not a valid bundle: {e}")))?;
    let scope: Scope = bundle.scope.parse()?;
    let target = workspace::resolve(scope, facts, resolved)?;
    target.ensure().map_err(WhisperError::from)?;

    let raw = std::fs::read_to_string(&target.path).map_err(WhisperError::from)?;
    let mut items = entry::parse_file(&raw);
    let mut added = 0usize;
    let mut duplicates_skipped = 0usize;

    let append_item = |items: &mut Vec<Item>, item: Item| -> bool {
        let duplicate = match &item {
            Item::Entry(e) => items
                .iter()
                .any(|i| matches!(i, Item::Entry(x) if x.id == e.id)),
            Item::Line(l) => items.iter().any(|i| matches!(i, Item::Line(x) if x == l)),
        };
        if !duplicate {
            items.push(item);
        }
        !duplicate
    };

    for e in bundle.entries {
        if append_item(&mut items, Item::Entry(e)) {
            added += 1;
        } else {
            duplicates_skipped += 1;
        }
    }
    for l in bundle.freeform {
        if append_item(&mut items, Item::Line(l)) {
            added += 1;
        } else {
            duplicates_skipped += 1;
        }
    }

    if added > 0 {
        std::fs::write(&target.path, entry::render_file(&items)).map_err(WhisperError::from)?;
    }

    Ok(json!({
        "target": target.path,
        "added": added,
        "duplicates_skipped": duplicates_skipped,
    }))
}

fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Repo => "repo",
        Scope::Branch => "branch",
        Scope::Worktree => "worktree",
        Scope::Group => "group",
    }
}

/// Read bundle input from a file or stdin.
pub fn read_input(file: Option<&str>, stdin: bool) -> Result<String> {
    if stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(WhisperError::from)?;
        return Ok(buf);
    }
    let path = file.ok_or_else(|| {
        WhisperError::new("no bundle input").with_suggestion("pass --file <path> or --stdin")
    })?;
    std::fs::read_to_string(path).map_err(WhisperError::from)
}
