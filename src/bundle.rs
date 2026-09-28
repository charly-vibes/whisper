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
    // Pack exclusion (add-repo-private-scope 2.3): the zone counterpart of
    // this scope is read only to report what was excluded — its items are
    // structurally outside the bundle (pack reads exactly the resolved
    // scope file, and private-resolved scopes are refused above).
    let private_excluded = private_zone_item_count(scope, facts, resolved);
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
        None => {
            let value = serde_json::to_value(&bundle)
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
    // Write guard (add-repo-private-scope 2.4): unpack never creates,
    // extends, or modifies files under the private zone. With the current
    // path-free bundle schema no shipped scope resolves there, but the
    // guard binds future routing changes: the content is skipped and
    // reported, never written.
    if workspace::is_in_private_zone(&target.path, facts) {
        let skipped = bundle.entries.len() + bundle.freeform.len();
        return Ok(json!({
            "target": target.path,
            "added": 0,
            "duplicates_skipped": 0,
            "private_skipped": skipped,
        }));
    }
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
        "private_skipped": serde_json::Value::Null,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn facts_with_local_root(root: &str) -> Facts {
        Facts {
            repo_key: "cv/charly-vibes/whisper".into(),
            branch_slug: "main".into(),
            worktree_slot: ".git".into(),
            repo_local_root: Some(PathBuf::from(root)),
        }
    }

    fn resolved_at(root: &std::path::Path) -> Resolved {
        Resolved {
            workspace_root: root.to_path_buf(),
            group: None,
            shadowed_global_root: None,
        }
    }

    fn one_entry_bundle() -> String {
        let bundle = Bundle {
            repo_key: "cv/charly-vibes/whisper".into(),
            scope: "repo".into(),
            entries: vec![Entry {
                ts: "2026-01-01T00:00:00Z".into(),
                id: "abc123".into(),
                topic: None,
                text: "bundled fact".into(),
                supersedes: None,
                superseded_by: None,
            }],
            freeform: vec!["loose line".into()],
        };
        serde_json::to_string(&bundle).unwrap()
    }

    // whisper-4xo (2.5): pack refuses any scope resolving under the private
    // zone — clear error, no bundle produced. No shipped scope resolves
    // there today; the trigger here is a misconfigured group root inside
    // the checkout's private zone, which resolve() faithfully honors.
    #[test]
    fn pack_refuses_a_scope_resolving_into_the_private_zone() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("bundle.json");
        let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().unwrap());
        let resolved = Resolved {
            workspace_root: tmp.path().join("ws"),
            group: Some(("rogue".into(), tmp.path().join(".whisper/private"))),
            shadowed_global_root: None,
        };

        let err = pack(Scope::Group, &facts, &resolved, Some(out.to_str().unwrap())).unwrap_err();
        assert!(err.to_string().contains("never transported"), "{err}");
        // No bundle produced.
        assert!(!out.exists());
    }

    // whisper-4xo (2.4): unpack never creates, extends, or modifies files
    // under the private zone — the content is skipped and reported.
    #[test]
    fn unpack_skips_and_reports_when_the_target_resolves_private() {
        let tmp = tempfile::tempdir().unwrap();
        let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().unwrap());
        let resolved = Resolved {
            workspace_root: tmp.path().join("ws"),
            group: Some(("rogue".into(), tmp.path().join(".whisper/private"))),
            shadowed_global_root: None,
        };
        let bundle = Bundle {
            repo_key: "cv/charly-vibes/whisper".into(),
            scope: "group".into(),
            entries: vec![Entry {
                ts: "2026-01-01T00:00:00Z".into(),
                id: "abc123".into(),
                topic: None,
                text: "bundled fact".into(),
                supersedes: None,
                superseded_by: None,
            }],
            freeform: vec!["loose line".into()],
        };
        let input = serde_json::to_string(&bundle).unwrap();

        let report = unpack(&input, &facts, &resolved).unwrap();
        assert_eq!(report["added"], 0);
        assert_eq!(report["private_skipped"], 2); // 1 entry + 1 freeform line
        // Nothing written under the zone.
        assert!(
            !tmp.path()
                .join(".whisper/private/repos/cv/charly-vibes/whisper/env.md")
                .exists()
        );
    }

    #[test]
    fn normal_pack_and_unpack_still_work_without_a_private_target() {
        let tmp = tempfile::tempdir().unwrap();
        let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().unwrap());
        let resolved = resolved_at(&tmp.path().join("ws").to_path_buf());

        let report = unpack(&one_entry_bundle(), &facts, &resolved).unwrap();
        assert_eq!(report["added"], 2);
        assert_eq!(report["private_skipped"], serde_json::Value::Null);
        assert!(tmp.path().join(".whisper/env.md").exists());
    }
}
