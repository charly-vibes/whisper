//! Compatibility fixture: verify Genesis-vibes v0.6 exposes the feedback APIs
//! the `turu feedback` subcommand delegates to (vampiro's pattern).

#[test]
fn genesis_api_feedback_args_importable() {
    let args = genesis::feedback::FeedbackArgs::new("bug", true, false);
    assert_eq!(args.kind, "bug");
    assert!(args.dry_run);
    assert!(!args.from_last_error);
}

#[test]
fn genesis_api_feedback_and_scratch_roundtrip() {
    // The unified handler must be callable with the documented signature
    // (genesis src/feedback.rs doc example). Seed the error scratch so the
    // from-last-error path is deterministic (stdin in the test harness is a
    // terminal, which the no-content path rejects). XDG_CACHE_HOME is shared
    // process state, so scratch write + read + feedback run in one test.
    use genesis::feedback::scratch::ErrorRecord;
    let cache = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_CACHE_HOME", cache.path()) };
    let record = ErrorRecord {
        ts: "2026-09-17T12:00:00Z".into(),
        argv: vec!["turu".into(), "recall".into(), "repo".into()],
        exit: 1,
        footer: None,
        kind: "error".into(),
    };
    genesis::feedback::scratch::write_scratch_best_effort("turu", &record);
    assert!(genesis::feedback::scratch::read_last_error("turu").is_some());
    let args = genesis::feedback::FeedbackArgs::new("bug", true, true);
    let result = genesis::feedback::handle_feedback(
        &args,
        "turu",
        "0.0.0-test",
        "charly-vibes/whisper",
        &std::env::temp_dir(),
    );
    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    // Dry-run returns the fallback URL without touching `gh`.
    match result {
        Ok(genesis::feedback::gh::GhResult::FallbackUrl(url)) => {
            assert!(url.contains("charly-vibes/whisper"));
        }
        _ => panic!("dry-run should return GhResult::FallbackUrl"),
    }
}
