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

/// Advisory incident-log lint (whisper-4xl): count date-like, id-like, and
/// chat-narration tokens in entry text; ≥3 per 500 chars (divisor floored
/// at 500 so short texts still need 3 tokens) suggests the text records an
/// incident instead of a generalizable lesson. Detection that warns can be
/// loose — this never blocks a write. Returns the advisory message.
/// Ro5-quantified lint thresholds (ticket whisper-4xl): ≥3 date/id-like
/// tokens per 500 chars ⇒ advisory. Divisor floored at 500 so short texts
/// still need 3 tokens. Tunable; detection that warns can be loose.
const LINT_TOKENS: f64 = 3.0;
const LINT_WINDOW: usize = 500;
const LINT_ID_MAX: usize = 64;

pub fn incident_log_density(text: &str) -> Option<String> {
    let tokens = text
        .split_whitespace()
        .filter(|t| is_date_like(t) || is_id_like(t) || is_issue_ref(t))
        .count();
    let units = (text.len() / LINT_WINDOW).max(1) as f64;
    if tokens as f64 / units < LINT_TOKENS {
        return None;
    }
    Some(format!(
        "this entry reads like an incident log ({tokens} date/id-like tokens in {} chars) — lessons, not logs: state the generalizable rule, keep one clause of why",
        text.len()
    ))
}

/// ISO-ish dates and timestamps: `2026-09-28`, `2026-09-28T10:00:00Z`,
/// `09/28/2026`. Bare times (`10:00`) stay quiet — 2–4 digits is not a date.
fn is_date_like(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let seps = t
        .chars()
        .filter(|c| *c == '-' || *c == '/' || *c == ':')
        .count();
    let has_year = t.len() >= 4 && t.chars().take(4).all(|c| c.is_ascii_digit());
    digits >= 6 && seps >= 1 && t.len() <= 20 && (seps >= 2 || has_year)
}

/// Hex-ish id: ≥8 hex chars INCLUDING at least one digit (so normal hex
/// words like `deadbeef`/`cafe` don't trip it), bounded length.
fn is_id_like(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    t.len() >= 8
        && t.len() <= LINT_ID_MAX
        && t.chars().all(|c| c.is_ascii_hexdigit())
        && t.chars().any(|c| c.is_ascii_digit())
}

/// `#123` (trailing punctuation tolerated) — `PR #123` via token split.
/// Leading `#` must survive trimming, so only trailing punctuation goes.
fn is_issue_ref(tok: &str) -> bool {
    let t = tok.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
    match t.strip_prefix('#') {
        Some(body) => !body.is_empty() && body.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
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

    #[test]
    fn incident_log_lint_flags_dense_date_id_text() {
        // Three tokens in a short text → over the 3-per-500 floor.
        let log = "fixed 2026-09-28 in PR #123 commit abc1234d verified";
        assert!(incident_log_density(log).is_some());
    }

    #[test]
    fn incident_log_lint_passes_generalizable_rules() {
        // One date in a short rule → quiet.
        assert!(
            incident_log_density(
                "deploy fails on tuesdays; retry after the queue drains (seen 2026-09-28)"
            )
            .is_none()
        );
        // A long rule with two dates spread over >500 chars stays quiet.
        let long_rule = format!(
            "Always regenerate Cargo.lock after a version bump before building with --locked; \
             the locked build fails otherwise. This bit the release pipeline twice. {} \
             Padding to push the token density under the threshold: the heuristic counts \
             date-like and id-like tokens per 500 characters of entry text, so a genuinely \
             long generalizable rule with a couple of incidental dates must never trip it. \
             Written 2026-09-28 after the v0.6.0 release cycle.",
            "x".repeat(300)
        );
        assert!(incident_log_density(&long_rule).is_none());
    }

    #[test]
    fn incident_log_lint_counts_dates_issue_refs_and_shas() {
        // ISO timestamp + issue ref + sha: three different token shapes.
        let mixed = "2026-01-01T10:00:00Z #456 broke main, hotfix b1a2c3d4 deployed";
        assert!(incident_log_density(mixed).is_some());
    }

    #[test]
    fn incident_log_lint_ignores_hex_words_without_digits() {
        // "deadbeef" and "cafe" are hex words but carry no digit — not ids.
        assert!(
            incident_log_density(
                "the deadbeef cafe served beef on a tuesday and the queue drained facedead"
            )
            .is_none()
        );
        // ...but abc1234d (digits) counts; three digit-bearing ids trip it.
        assert!(
            incident_log_density(
                "reverted abc1234d then rebased deadbeef onto d1e2f3a4b5c6d7e picking 9f8e7d6c"
            )
            .is_some()
        );
    }
}
