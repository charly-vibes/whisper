//! Append/ingest semantics: entry structure, ids, normalization, supersedes.

use crate::common::{Uw, at, extract_id, repo_env, resolve_repo_path};
use predicates::str::contains;

#[test]
pub(crate) fn append_fast_path_preserves_file_bytes_verbatim() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let path = resolve_repo_path(&home, &repo);
    std::fs::create_dir_all(path.parent().uw()).uw();
    // Legacy file with non-canonical content (odd id width, no trailing newline).
    std::fs::write(&path, "- 2026-01-01T00:00:00Z [id:short] legacy weird").uw();
    let before = std::fs::read(&path).uw();

    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args(["append", "repo", "--text", "new fact"])
        .assert()
        .success();

    let after = std::fs::read(&path).uw();
    // Fast path: pure O_APPEND — the original bytes must be untouched.
    assert_eq!(&after[..before.len()], &before[..]);
    assert!(after.ends_with(b"new fact\n"));
}

#[test]
pub(crate) fn append_is_idempotent_within_the_same_second() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    for _ in 0..2 {
        at(&home, &repo, "2026-01-01T00:00:00Z")
            .args(["append", "repo", "--text", "same fact", "--json"])
            .assert()
            .success();
    }
    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    assert_eq!(
        content.lines().filter(|l| l.contains("same fact")).count(),
        1,
        "file must contain exactly one copy: {content}"
    );

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "same fact", "--json"])
        .assert()
        .stdout(contains("\"duplicate\":true"));
}

#[test]
pub(crate) fn append_stays_quiet_on_generalizable_rules() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let out = at(&home, &repo, "2026-01-01T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "deploy fails on tuesdays; retry after the queue drains",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"warnings\":[]"));
    assert_eq!(out.get_output().status.code(), Some(0));
}

#[test]
pub(crate) fn append_warns_on_incident_log_shaped_text() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "fixed the 2026-09-28 outage in PR #123, commit abc1234d verified on main",
            "--json",
        ])
        .assert()
        .success() // advisory only: exit 0, write proceeds
        .stdout(contains("\"warnings\""))
        .stdout(contains("lessons, not logs"));

    // The write itself is unchanged.
    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    assert!(content.contains("outage"));
}

#[test]
pub(crate) fn append_writes_structured_entry_with_id() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "deploy fails on tuesday",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"duplicate\":false"))
        .stdout(contains("\"id\":\""));

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    let line = content.lines().find(|l| l.starts_with("- ")).uw();
    assert!(line.starts_with("- 2026-01-01T00:00:00Z [id:"));
    assert!(line.ends_with("deploy fails on tuesday"));
}

#[test]
pub(crate) fn appended_bytes_reports_the_rendered_line_not_the_raw_input() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    // Raw input carries trailing whitespace that is trimmed at ingest.
    let out = String::from_utf8(
        at(&home, &repo, "2026-01-01T00:00:00Z")
            .args(["append", "repo", "--text", "fact with padding   ", "--json"])
            .output()
            .uw()
            .stdout,
    )
    .uw();
    let reported = crate::common::json_num(&out, "appended_bytes");
    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    let line = content
        .lines()
        .find(|l| l.contains("fact with padding"))
        .uw();
    assert_eq!(reported as usize, line.len() + 1); // line + trailing newline
}

#[test]
pub(crate) fn code_blocks_after_entries_are_not_absorbed_as_continuations() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let path = resolve_repo_path(&home, &repo);
    std::fs::create_dir_all(path.parent().uw()).uw();
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "first"])
        .assert()
        .success();
    let entry_len = std::fs::read_to_string(&path).uw().len();
    std::fs::write(
        &path,
        format!(
            "{}\n    let x = indented_code();\n",
            std::fs::read_to_string(&path).uw().trim_end()
        ),
    )
    .uw();
    let with_code = std::fs::read_to_string(&path).uw();
    assert!(with_code[entry_len..].starts_with("    let"));

    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args(["append", "repo", "--text", "second"])
        .assert()
        .success();

    let after = std::fs::read_to_string(&path).uw();
    // The 4-space code line survives verbatim (not folded into the entry).
    assert!(after.contains("    let x = indented_code();"));
}

#[test]
pub(crate) fn freeform_lines_are_preserved_verbatim() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    let path = resolve_repo_path(&home, &repo);
    std::fs::create_dir_all(path.parent().uw()).uw();
    std::fs::write(&path, "## notes\n\n- a plain bullet\n").uw();

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "a fact"])
        .assert()
        .success();

    let content = std::fs::read_to_string(&path).uw();
    assert!(content.contains("## notes"));
    assert!(content.contains("- a plain bullet"));
    assert!(content.contains("a fact"));
}

#[test]
pub(crate) fn multi_line_text_with_leading_whitespace_normalizes_at_ingest() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);
    let text = "steps:\n    one indented\n        two deeper";

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", text, "--json"])
        .assert()
        .success();

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    assert!(content.contains("steps:\n  one indented\n  two deeper"));

    // Re-appending the same raw text is a duplicate: the id is computed
    // over the NORMALIZED text.
    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", text, "--json"])
        .assert()
        .stdout(contains("\"duplicate\":true"));
}

#[test]
pub(crate) fn restatement_later_is_a_new_entry() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "stated twice"])
        .assert()
        .success();
    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args(["append", "repo", "--text", "stated twice"])
        .assert()
        .success();

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    assert_eq!(
        content
            .lines()
            .filter(|l| l.contains("stated twice") && l.contains("[id:"))
            .count(),
        2
    );
}

#[test]
pub(crate) fn supersedes_marks_the_target_and_records_the_reference() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "old fact", "--json"])
        .assert()
        .success();
    let first_id = extract_id(&resolve_repo_path(&home, &repo), "old fact");

    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "new fact",
            "--supersedes",
            &first_id,
            "--json",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw();
    let old = content.lines().find(|l| l.contains("old fact")).uw();
    assert!(
        old.contains(&format!("(superseded by {first_id})")) || old.contains("(superseded by ")
    );
    let new_line = content.lines().find(|l| l.contains("new fact")).uw();
    assert!(new_line.contains(&format!("(supersedes {first_id})")));
}

#[test]
pub(crate) fn supersedes_unknown_id_fails_without_touching_the_file() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "kept fact"])
        .assert()
        .success();
    let before = std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw();

    at(&home, &repo, "2026-01-02T00:00:00Z")
        .args([
            "append",
            "repo",
            "--text",
            "new",
            "--supersedes",
            &"f".repeat(64),
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));

    let after = std::fs::read_to_string(resolve_repo_path(&home, &repo)).uw();
    assert_eq!(before, after);
}

#[test]
pub(crate) fn topic_lands_in_the_entry_line() {
    let tmp = tempfile::tempdir().uw();
    let (home, repo) = repo_env(&tmp);

    at(&home, &repo, "2026-01-01T00:00:00Z")
        .args(["append", "repo", "--text", "use kaniko", "--topic", "infra"])
        .assert()
        .success();

    let path = resolve_repo_path(&home, &repo);
    let content = std::fs::read_to_string(&path).uw();
    assert!(content.contains(" (#infra) use kaniko"));
}
