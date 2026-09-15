//! Structured knowledge entries: the machine-addressable ledger line format
//! for scope files.
//!
//! An entry is one markdown line (continuation lines start with two spaces):
//!
//! `- <RFC3339-seconds-UTC> [id:<sha2-hex>] (#topic)? text… (supersedes <id>)?`
//!
//! plus an optional trailing ` (superseded by <id>)` marker. Lines that do
//! not parse as entries (including blanks) are unmanaged freeform and are
//! passed through verbatim — `turu` never interprets their content.

use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description;
use time::macros::format_description;

use crate::{Result, WhisperError};

/// Second-precision UTC timestamp format: `2026-09-15T19:30:02Z`.
pub fn ts_format() -> format_description::FormatItem<'static> {
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z").into()
}

/// Current UTC timestamp at second precision, or the explicit override
/// when one is passed (used by `TURU_NOW` for reproducible runs).
pub fn parse_or_now(override_ts: Option<&str>) -> Result<String> {
    if let Some(raw) = override_ts {
        // Accept any RFC-3339 form, normalize to second-precision UTC.
        let dt = OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
            .map_err(|_| {
                WhisperError::new(format!("timestamp override is not RFC-3339: {raw}"))
                    .with_suggestion("pass TURU_NOW like 2026-01-01T00:00:00Z, or unset it")
            })?;
        return dt
            .format(&ts_format())
            .map_err(|e| WhisperError::new(format!("formatting timestamp: {e}")));
    }
    OffsetDateTime::now_utc()
        .format(&ts_format())
        .map_err(|e| WhisperError::new(format!("formatting timestamp: {e}")))
}

/// Full sha2 hex of `s` — the id alphabet for entries, revisions, bundles.
pub fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let d = h.finalize();
    let mut out = String::with_capacity(d.len() * 2);
    for b in d {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Deterministic entry id: sha2 of `scope_key + timestamp + content`.
///
/// Identical text within the same second in the same scope yields the same
/// id (the idempotency guarantee); a later re-statement is a new entry.
pub fn entry_id(scope_key: &str, ts: &str, text: &str) -> String {
    sha256_hex(&format!("{scope_key}\n{ts}\n{text}"))
}

/// A parsed structured entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub ts: String,
    pub id: String,
    pub topic: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

impl Entry {
    /// Render as one ledger line (multi-line text becomes two-space
    /// continuation lines); suffix markers go after the final text line.
    pub fn render(&self) -> String {
        let mut line = format!("- {} [id:{}]", self.ts, self.id);
        if let Some(t) = &self.topic {
            line.push_str(&format!(" (#{t})"));
        }
        line.push(' ');
        let mut lines = self.text.split('\n');
        line.push_str(lines.next().unwrap_or(""));
        for cont in lines {
            line.push_str("\n  ");
            line.push_str(cont);
        }
        if let Some(s) = &self.supersedes {
            line.push_str(&format!(" (supersedes {s})"));
        }
        if let Some(s) = &self.superseded_by {
            line.push_str(&format!(" (superseded by {s})"));
        }
        line
    }
}

/// One item of a scope file, in file order.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Entry(Entry),
    /// Unmanaged text (including blank lines) — passed through verbatim.
    Line(String),
}

/// Parse a line that starts an entry (continuation lines handled by the
/// caller). Returns `None` for any non-entry line.
fn parse_entry_line(line: &str) -> Option<Entry> {
    let rest = line.strip_prefix("- ")?;
    let (ts, after) = rest.split_once(' ')?;
    // Timestamp must be canonical RFC-3339 seconds — malformed or
    // non-canonical lines degrade to unmanaged freeform instead of
    // corrupting recency ordering.
    match OffsetDateTime::parse(ts, &time::format_description::well_known::Rfc3339) {
        Ok(dt) if dt.format(&ts_format()).is_ok_and(|f| f == ts) => {}
        _ => return None,
    }
    let id_tok = after.strip_prefix("[id:")?;
    let (id, after) = id_tok.split_once(']')?;
    let (topic, after) = if let Some(stripped) = after.strip_prefix(" (#") {
        let (t, tail) = stripped.split_once(')')?;
        (Some(t.to_string()), tail.strip_prefix(' ').unwrap_or(tail))
    } else {
        (None, after.strip_prefix(' ').unwrap_or(after))
    };
    let mut text = after.to_string();
    let mut superseded_by = None;
    if let Some(idx) = text.rfind(" (superseded by ") {
        let suffix = &text[idx..];
        if suffix.ends_with(')') {
            superseded_by = Some(suffix[" (superseded by ".len()..suffix.len() - 1].to_string());
            text.truncate(idx);
        }
    }
    let mut supersedes = None;
    if let Some(idx) = text.rfind(" (supersedes ") {
        let suffix = &text[idx..];
        if suffix.ends_with(')') {
            supersedes = Some(suffix[" (supersedes ".len()..suffix.len() - 1].to_string());
            text.truncate(idx);
        }
    }
    Some(Entry {
        ts: ts.to_string(),
        id: id.to_string(),
        topic,
        text,
        supersedes,
        superseded_by,
    })
}

/// Parse a whole scope file into ordered items. Continuation lines
/// (starting with exactly two spaces) fold into the preceding entry's text.
pub fn parse_file(raw: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        match parse_entry_line(line) {
            Some(mut entry) => {
                // Continuation lines start with EXACTLY two spaces (the
                // renderer's convention) — deeper indents (e.g. markdown
                // code blocks) stay unmanaged freeform.
                while lines.peek().is_some_and(|l| {
                    l.starts_with("  ") && !l.starts_with("   ") && !l.trim().is_empty()
                }) {
                    let cont = lines.next().unwrap();
                    entry.text.push('\n');
                    entry.text.push_str(cont.trim_start());
                }
                items.push(Item::Entry(entry));
            }
            None => items.push(Item::Line(line.to_string())),
        }
    }
    items
}

/// Render items back to file content (entries canonically, lines verbatim).
pub fn render_file(items: &[Item]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            Item::Entry(e) => {
                out.push_str(&e.render());
                out.push('\n');
            }
            Item::Line(l) => {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}

/// All entry ids in a file's items.
pub fn entry_ids(items: &[Item]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Entry(e) => Some(e.id.as_str()),
            Item::Line(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-15T19:30:02Z";

    fn entry(text: &str) -> Entry {
        Entry {
            ts: NOW.to_string(),
            id: entry_id("github.com/u/r", NOW, text),
            topic: None,
            text: text.to_string(),
            supersedes: None,
            superseded_by: None,
        }
    }

    #[test]
    fn malformed_ts_degrades_to_freeform() {
        assert!(parse_entry_line("- banana [id:abc] text").is_none());
        assert!(parse_entry_line("- 2026-13-45T99:99:99Z [id:abc] text").is_none());
        assert!(parse_entry_line("- 2026-01-01T00:00:00.123Z [id:abc] subsecond").is_none());
        assert!(parse_entry_line("- 2026-01-01T00:00:00Z [id:abc] ok").is_some());
    }

    #[test]
    fn ts_is_second_precision_utc() {
        let ts = parse_or_now(None).unwrap();
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert!(ts.starts_with("20"));
    }

    #[test]
    fn timestamp_override_is_validated() {
        assert!(parse_or_now(Some("not-a-date")).is_err());
        assert_eq!(
            parse_or_now(Some("2026-01-01T00:00:00Z")).unwrap(),
            "2026-01-01T00:00:00Z"
        );
    }

    #[test]
    fn id_is_deterministic_per_scope_ts_text() {
        let a = entry_id("s", NOW, "text");
        assert_eq!(a, entry_id("s", NOW, "text"));
        assert_ne!(a, entry_id("s", NOW, "other"));
        assert_ne!(a, entry_id("s", "2026-09-15T19:30:03Z", "text"));
        assert_ne!(a, entry_id("t", NOW, "text"));
        assert_eq!(a.len(), 64); // full sha2 hex, no truncation
    }

    #[test]
    fn render_parse_roundtrip() {
        let e = entry("deploy fails on tuesday");
        assert_eq!(parse_entry_line(&e.render()).unwrap(), e);
    }

    #[test]
    fn render_parse_roundtrip_with_topic_and_marks() {
        let mut e = entry("use kaniko not docker");
        e.topic = Some("infra".into());
        e.supersedes = Some("aa".repeat(64));
        e.superseded_by = Some("bb".repeat(64));
        assert_eq!(parse_entry_line(&e.render()).unwrap(), e);
    }

    #[test]
    fn freeform_lines_never_parse_as_entries() {
        assert!(parse_entry_line("- a plain bullet").is_none());
        assert!(parse_entry_line("## heading").is_none());
        assert!(parse_entry_line("").is_none());
        assert!(parse_entry_line("- 2026-09-15T19:30:02Z no id marker").is_none());
    }

    #[test]
    fn file_roundtrip_preserves_order_and_freeform() {
        let raw = "## notes\n\n- plain bullet\n";
        let e = entry("a fact");
        let raw = format!("{raw}{}\n\n- another plain\n", e.render());
        let items = parse_file(&raw);
        assert_eq!(
            items,
            vec![
                Item::Line("## notes".into()),
                Item::Line(String::new()),
                Item::Line("- plain bullet".into()),
                Item::Entry(e),
                Item::Line(String::new()),
                Item::Line("- another plain".into()),
            ]
        );
        assert_eq!(render_file(&parse_file(&raw)), raw);
    }

    #[test]
    fn continuation_lines_fold_into_text() {
        let mut e = entry("line one\nline two");
        e.topic = Some("t".into());
        let raw = e.render();
        let items = parse_file(&raw);
        match &items[0] {
            Item::Entry(parsed) => {
                assert_eq!(parsed.text, "line one\nline two");
                assert_eq!(parsed.topic.as_deref(), Some("t"));
            }
            _ => panic!("expected entry"),
        }
    }
}
