//! Shared-store primitive: deterministic, transport-agnostic knowledge
//! bundles keyed by the canonical repo key. Packing identical state yields
//! identical bytes; unpack merges extend-without-duplicate (the
//! `consolidate` rules) and never overwrites existing content. Transport
//! guards: the checkout's private zone is refused at pack time, skipped at
//! unpack time, and never enters a bundle.

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
    // Transport guard (add-repo-private-scope 2.5): no shipped scope
    // resolves into the private zone, but the refusal binds future routing
    // changes — the zone is never carried, no exceptions, no bundle.
    if workspace::is_in_private_zone(&target.path, facts) {
        return Err(
            WhisperError::new("the private zone is never transported — refusing to pack")
                .with_suggestion("pack a scope resolving outside the checkout's .whisper/private/"),
        );
    }
    let (mut entries, freeform) = read_scope_items(&target.path);
    // Pack exclusion (add-repo-private-scope 2.3): the zone counterpart of
    // this scope is read only to report what was excluded — its items are
    // structurally outside the bundle (pack reads exactly the resolved
    // scope file, and private-resolved scopes are refused above).
    let private_excluded = private_zone_item_count(scope, facts, resolved);
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let bundle = assemble_bundle(scope, facts, entries, freeform);
    match out {
        Some(path) => pack_to_file(&bundle, path, private_excluded),
        None => pack_inline(&bundle, private_excluded),
    }
}

/// Assemble the transport bundle (sorted entries, addressing included).
fn assemble_bundle(
    scope: Scope,
    facts: &Facts,
    entries: Vec<Entry>,
    freeform: Vec<String>,
) -> Bundle {
    Bundle {
        repo_key: facts.repo_key.clone(),
        scope: scope_name(scope).to_string(),
        entries,
        freeform,
    }
}

/// Read the resolved scope file into (entries, freeform) items.
fn read_scope_items(path: &std::path::Path) -> (Vec<Entry>, Vec<String>) {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let mut entries: Vec<Entry> = Vec::new();
    let mut freeform: Vec<String> = Vec::new();
    for item in entry::parse_file(&raw) {
        match item {
            Item::Entry(e) => entries.push(e),
            Item::Line(l) if !l.trim().is_empty() => freeform.push(l),
            Item::Line(_) => {}
        }
    }
    (entries, freeform)
}

/// Write the bundle to `path` (creating parent dirs) and report its digest.
fn pack_to_file(bundle: &Bundle, path: &str, private_excluded: usize) -> Result<serde_json::Value> {
    let bytes = serde_json::to_string_pretty(bundle)
        .map_err(|e| WhisperError::new(format!("serializing bundle: {e}")))?;
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(WhisperError::from)?;
    }
    std::fs::write(path, &bytes).map_err(WhisperError::from)?;
    Ok(json!({
        "path": path,
        "sha256": entry::sha256_hex(&bytes),
        "repo_key": bundle.repo_key,
        "scope": bundle.scope,
        "entries": bundle.entries.len(),
        "freeform_lines": bundle.freeform.len(),
        "private_excluded": private_excluded,
    }))
}

/// Return the bundle inline instead of writing it to a file.
fn pack_inline(bundle: &Bundle, private_excluded: usize) -> Result<serde_json::Value> {
    let value = serde_json::to_value(bundle)
        .map_err(|e| WhisperError::new(format!("serializing bundle: {e}")))?;
    Ok(json!({
        "bundle": value,
        "repo_key": bundle.repo_key,
        "scope": bundle.scope,
        "entries": bundle.entries.len(),
        "freeform_lines": bundle.freeform.len(),
        "private_excluded": private_excluded,
    }))
}

/// `turu bundle unpack`: merge a bundle into the local workspace using
/// extend-without-duplicate rules; entry ids already present are skipped
/// and reported; existing file content is never overwritten.
pub fn unpack(input: &str, facts: &Facts, resolved: &Resolved) -> Result<serde_json::Value> {
    let bundle: Bundle = serde_json::from_str(input)
        .map_err(|e| WhisperError::new(format!("not a valid bundle: {e}")))?;
    let scope: Scope = bundle.scope.parse()?;
    let target = workspace::resolve(scope, facts, resolved)?;
    // Write guard (add-repo-private-scope 2.4): unpack never creates,
    // extends, or modifies files under the private zone. With the current
    // path-free bundle schema no shipped scope resolves there, but the
    // guard binds future routing changes: the content is skipped and
    // reported, never written.
    if workspace::is_in_private_zone(&target.path, facts) {
        return Ok(private_skip_report(&target.path, &bundle));
    }
    target.ensure().map_err(WhisperError::from)?;
    merge_bundle(&bundle, &target.path)
}

/// Report for a bundle whose scope resolves into the private zone: the
/// content is skipped and reported, never written (add-repo-private-scope
/// 2.4 write guard).
fn private_skip_report(path: &std::path::Path, bundle: &Bundle) -> serde_json::Value {
    let skipped = bundle.entries.len() + bundle.freeform.len();
    json!({
        "target": path,
        "added": 0,
        "duplicates_skipped": 0,
        "private_skipped": skipped,
    })
}

/// Extend-only merge: append bundle items that are not already present
/// (deduplicated by entry id / exact line) and rewrite the scope file when
/// anything was added. Returns the unpack report.
fn merge_bundle(bundle: &Bundle, path: &std::path::Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path).map_err(WhisperError::from)?;
    let mut items = entry::parse_file(&raw);
    let mut added = 0usize;
    let mut duplicates_skipped = 0usize;

    for item in bundle_items(bundle) {
        if append_item(&mut items, item) {
            added += 1;
        } else {
            duplicates_skipped += 1;
        }
    }

    if added > 0 {
        std::fs::write(path, entry::render_file(&items)).map_err(WhisperError::from)?;
    }

    Ok(json!({
        "target": path,
        "added": added,
        "duplicates_skipped": duplicates_skipped,
        "private_skipped": serde_json::Value::Null,
    }))
}

/// The bundle's items in serving order: entries first, then freeform lines.
fn bundle_items(bundle: &Bundle) -> impl Iterator<Item = Item> + '_ {
    bundle
        .entries
        .iter()
        .cloned()
        .map(Item::Entry)
        .chain(bundle.freeform.iter().cloned().map(Item::Line))
}

/// Append `item` unless already present (entry id / exact line match).
fn append_item(items: &mut Vec<Item>, item: Item) -> bool {
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
}

/// Items (entries + freeform lines) in the private-zone counterpart of
/// `scope` — the count reported as excluded by pack. Zero outside a
/// checkout, for scopes without a zone counterpart, or when the file does
/// not exist yet.
fn private_zone_item_count(scope: Scope, facts: &Facts, resolved: &Resolved) -> usize {
    workspace::private_zone_target(scope, facts, resolved)
        .ok()
        .and_then(|t| std::fs::read_to_string(t.path).ok())
        .map(|raw| {
            entry::parse_file(&raw)
                .iter()
                .filter(|i| match i {
                    Item::Entry(_) => true,
                    Item::Line(l) => !l.trim().is_empty(),
                })
                .count()
        })
        .unwrap_or(0)
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
