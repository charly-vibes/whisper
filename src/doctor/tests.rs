//! Doctor unit tests (leak lint + helpers).

use super::leaks::leak_findings;

#[test]
fn leak_lint_flags_machine_paths() {
    for p in [
        "/home/sasha/infra",
        "/Users/sasha/infra",
        "/var/home/sasha/infra",
        "C:\\Users\\sasha",
    ] {
        let findings = leak_findings(&format!("deploy runs from {p}"));
        assert!(
            findings.iter().any(|m| m.contains("machine path")),
            "{p}: {findings:?}"
        );
    }
}

#[test]
fn leak_lint_flags_hostname_assignments() {
    let findings = leak_findings("built on host: sasha-fedora and hostname=brix");
    assert!(
        findings.iter().any(|m| m.contains("hostname")),
        "{findings:?}"
    );
}

#[test]
fn leak_lint_flags_token_shaped_strings() {
    for tok in [
        "ghp_0123456789abcdef0123456789abcdef0123",
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    ] {
        let findings = leak_findings(&format!("token {tok}"));
        assert!(
            findings.iter().any(|m| m.contains("token-shaped")),
            "{tok}: {findings:?}"
        );
    }
}

#[test]
fn leak_lint_stays_quiet_on_ordinary_lessons() {
    assert!(
        leak_findings("prefer O_APPEND fast path; full rewrite only for --supersedes").is_empty()
    );
    assert!(leak_findings("entry ids are sha256 over scope-key+ts+text").is_empty());
    assert!(leak_findings("ts 2026-09-28T21:48:17Z").is_empty());
    // Hex words without digits never trip the token shape.
    assert!(leak_findings("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef is not a token").is_empty());
}

#[test]
fn leak_lint_reports_line_numbers() {
    let findings = leak_findings("ok line\n/home/sasha/infra here");
    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("line 2"), "{findings:?}");
    assert!(findings[0].contains("/home/sasha/infra"), "{findings:?}");
}
