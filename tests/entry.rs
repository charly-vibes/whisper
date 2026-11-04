//! Integration tests for entry parsing/rendering (public API only).

use whisper::entry::{
    Entry, Item, entry_id, incident_log_density, parse_entry_line, parse_file, parse_or_now,
    render_file,
};

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
fn render_parse_roundtrip_covers_plain_topic_and_marks() {
    // Plain entry: the render round-trips exactly.
    let plain = entry("deploy fails on tuesday");
    let parsed = parse_entry_line(&plain.render()).unwrap();
    assert_eq!(parsed, plain);
    assert_eq!(parsed.topic, None);
    assert_eq!(parsed.supersedes, None);
    assert_eq!(parsed.superseded_by, None);
    // Topic + both supersede markers round-trip too.
    let mut marked = entry("use kaniko not docker");
    marked.topic = Some("infra".into());
    marked.supersedes = Some("aa".repeat(64));
    marked.superseded_by = Some("bb".repeat(64));
    let parsed_marked = parse_entry_line(&marked.render()).unwrap();
    assert_eq!(parsed_marked, marked);
    assert_eq!(parsed_marked.topic.as_deref(), Some("infra"));
    assert_eq!(
        parsed_marked.superseded_by.as_deref(),
        Some("bb".repeat(64).as_str())
    );
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
    assert_eq!(items.len(), 6, "every line is preserved, nothing dropped");
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
    assert_eq!(items.len(), 1, "folded lines stay one entry");
}

#[test]
fn incident_log_lint_flags_dense_date_id_text_over_the_per_500_floor() {
    // Three tokens in a short text → over the 3-per-500 floor; the reason
    // is non-empty, and a single token below the floor stays quiet.
    let log = "fixed 2026-09-28 in PR #123 commit abc1234d verified";
    let reason = match incident_log_density(log) {
        Some(r) => r,
        None => panic!("dense log must be flagged"),
    };
    assert!(!reason.is_empty());
    assert!(
        incident_log_density("fixed 2026-09-28 in the release pipeline").is_none(),
        "one token must stay under the floor"
    );
    let two = incident_log_density("fixed 2026-09-28 sha abc1234d verified in staging");
    assert!(two.is_none(), "two tokens must stay under the floor");
    assert!(two == incident_log_density("no tokens at all here"));
}

#[test]
fn incident_log_lint_stays_quiet_for_generalizable_rules_with_sparse_dates() {
    // One date in a short rule → quiet.
    assert!(
        incident_log_density(
            "deploy fails on tuesdays; retry after the queue drains (seen 2026-09-28)"
        )
        .is_none()
    );
    // Two dates in a short rule stay under the floor too.
    let two_dates = "bump on 2026-09-28, verify on 2026-09-30, then ship";
    assert!(incident_log_density(two_dates).is_none());
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
fn incident_log_lint_mixed_token_shapes_timestamp_issue_ref_sha() {
    // ISO timestamp + issue ref + sha: three different token shapes.
    let mixed = "2026-01-01T10:00:00Z #456 broke main, hotfix b1a2c3d4 deployed";
    let reason = match incident_log_density(mixed) {
        Some(r) => r,
        None => panic!("mixed token shapes must be flagged"),
    };
    assert!(!reason.is_empty());
    // Dropping any one token shape drops below the floor.
    assert!(
        incident_log_density("#456 broke main, hotfix b1a2c3d4 deployed").is_none(),
        "two tokens must stay under the floor"
    );
    // Dropping two leaves one: quiet with the same None verdict shape.
    match incident_log_density("#456 broke main and was reverted") {
        None => {}
        Some(r) => panic!("single token must not trip the lint: {r}"),
    }
}

#[test]
fn incident_log_lint_hex_words_without_digits_are_not_ids() {
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
