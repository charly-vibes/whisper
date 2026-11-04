//! Leak-shape lint (add-repo-private-scope 3.2c) — advisory: machine
//! paths, hostname assignments, and token-shaped strings in public files.

use crate::config::Resolved;
use crate::workspace::{Facts, Scope, resolve};

/// Machine-path prefixes that betray a specific developer machine.
const LEAK_PATH_PREFIXES: [&str; 4] = ["C:\\Users\\", "/var/home/", "/home/", "/Users/"];

/// Minimum token length for the token-shaped shape.
const LEAK_TOKEN_MIN: usize = 32;

/// Advisory leak-shape lint over one text: flag machine paths, hostname
/// assignments, and token-shaped strings (API keys, PATs, long hex ids).
/// Detection that warns can be loose; this never blocks anything.
/// Findings carry 1-based line numbers relative to the linted text.
pub fn leak_findings(text: &str) -> Vec<String> {
    let mut findings = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line_no = n + 1;
        for prefix in LEAK_PATH_PREFIXES {
            if let Some(at) = machine_path_at(line, prefix) {
                findings.push(format!(
                    "line {line_no}: machine path `{}`",
                    path_at(&line[at..])
                ));
                break;
            }
        }
        if let Some(found) = hostname_assignment(line) {
            findings.push(format!("line {line_no}: hostname assignment `{found}`"));
        }
        for tok in line.split_whitespace() {
            let t =
                tok.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));
            if is_token_shaped(t) {
                let shown: String = t.chars().take(16).collect();
                findings.push(format!(
                    "line {line_no}: token-shaped string `{shown}…` ({} chars)",
                    t.len()
                ));
            }
        }
    }
    findings
}

/// Advisory leak warnings for the doctor envelope: lint the public
/// repo-local knowledge files (repo env.md, branch notes.md). Entry text
/// is linted, never raw file lines — entry markers carry ids/timestamps
/// that would false-positive every well-formed file (the whisper-4xl
/// lesson); freeform lines are user prose and are linted as-is.
pub fn leak_warnings(facts: &Facts, resolved: &Resolved) -> Vec<String> {
    if facts.repo_local_root.is_none() {
        return Vec::new();
    }
    [Scope::Repo, Scope::Branch]
        .iter()
        .filter_map(|scope| lint_scope(*scope, facts, resolved))
        .collect()
}

/// Lint one public scope file; None when it resolves nowhere public.
fn lint_scope(scope: Scope, facts: &Facts, resolved: &Resolved) -> Option<String> {
    let target = resolve(scope, facts, resolved).ok()?;
    if crate::workspace::is_in_private_zone(&target.path, facts) || !target.path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(&target.path).ok()?;
    let linted = lint_items(crate::entry::parse_file(&raw));
    if linted.is_empty() {
        None
    } else {
        Some(format!(
            "leak-shaped content in `{}`: {}",
            target.path.display(),
            linted.join(", ")
        ))
    }
}

/// Lint parsed scope-file items. Superseded entries are dead by decision —
/// the staleness check exempts them and so does the leak lint: their text
/// stays in the file but must not nag forever. Freeform lines are user
/// prose and are linted as-is.
fn lint_items(items: Vec<crate::entry::Item>) -> Vec<String> {
    let mut linted = Vec::new();
    for item in items {
        match item {
            crate::entry::Item::Entry(e) if e.superseded_by.is_some() => {}
            crate::entry::Item::Entry(e) => linted.extend(leak_findings(&e.text)),
            crate::entry::Item::Line(l) => {
                if !l.trim().is_empty() {
                    linted.extend(leak_findings(&l));
                }
            }
        }
    }
    linted
}

/// Start of a machine path at a word boundary (not inside another word —
/// `/var/home/` contains `/home/` but only as a non-boundary suffix).
fn machine_path_at(line: &str, prefix: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(idx) = line[from..].find(prefix) {
        let at = from + idx;
        let boundary = at == 0
            || !line[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
        if boundary {
            return Some(at);
        }
        from = at + prefix.len().max(1);
    }
    None
}

/// Path string starting at `at`: extends over alphanumerics and `/-_.~`.
fn path_at(rest: &str) -> String {
    rest.chars()
        .take_while(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~' | '\\')
        })
        .collect()
}

/// A `host:`/`hostname=`-shaped assignment: the word `host` (optionally
/// `name`), optional spaces, `:` or `=`, then a value. Word-bounded so
/// `localhost:8080` stays quiet. Returns the assigned value.
fn hostname_assignment(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let mut from = 0;
    while let Some(idx) = lower[from..].find("host") {
        let at = from + idx;
        let boundary = at == 0
            || !lower[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
        if boundary {
            if let Some(v) = hostname_value(&lower[at + 4..]) {
                return Some(v);
            }
        }
        from = at + 4;
    }
    None
}

/// The assigned value after `host`/`hostname` and a `:`/`=` separator.
fn hostname_value(after: &str) -> Option<String> {
    let after = after.strip_prefix("name").unwrap_or(after);
    let value_part = after.trim_start();
    let v = value_part
        .strip_prefix(':')
        .or_else(|| value_part.strip_prefix('='))?
        .trim();
    if v.is_empty() {
        return None;
    }
    let end = v.find(char::is_whitespace).unwrap_or(v.len());
    Some(v[..end].to_string())
}

/// Token-shaped: long (≥32) run of alphanumerics/`_-` containing both a
/// digit and a letter — API keys, PATs, long hex ids. Hex words without
/// digits (`deadbeef…`) stay quiet.
fn is_token_shaped(tok: &str) -> bool {
    tok.len() >= LEAK_TOKEN_MIN
        && tok
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && tok.chars().any(|c| c.is_ascii_digit())
        && tok.chars().any(|c| c.is_ascii_alphabetic())
}

pub(super) fn short_id(id: &str) -> &str {
    &id[..id.len().min(12)]
}

pub(super) fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Repo => "repo",
        Scope::Branch => "branch",
        Scope::Worktree => "worktree",
        Scope::Group => "group",
    }
}
