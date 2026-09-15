//! Recall serving: ranked, budget-bounded slices of scope entries.
//!
//! turu serves mechanically (filter, rank, budget); *when* to call recall
//! is the agent's policy. Whole-entry atomicity: an entry that does not fit
//! the budget is skipped in its entirety, never truncated.

use serde::Serialize;
use serde_json::{Value, json};

use crate::config::Resolved;
use crate::entry::{self, Item};
use crate::workspace::{self, Facts, Scope};
use crate::{Result, WhisperError};

/// Recall scope argument: one routing scope, or `all` (composition of every
/// applicable scope in precedence order). `all` is a recall argument — the
/// routing `Scope` enum is untouched.
#[derive(Debug, Clone, PartialEq)]
pub enum RecallScope {
    One(Scope),
    All,
}

pub fn parse_recall_scope(s: &str) -> Result<RecallScope> {
    if s.eq_ignore_ascii_case("all") {
        Ok(RecallScope::All)
    } else {
        Ok(RecallScope::One(s.parse()?))
    }
}

/// A served entry, tagged with its scope.
#[derive(Debug, Serialize)]
pub struct Served {
    pub scope: &'static str,
    #[serde(flatten)]
    pub entry: entry::Entry,
}

/// A served unmanaged line, tagged with its scope.
#[derive(Debug, Serialize)]
pub struct ServedLine {
    pub scope: &'static str,
    pub line: String,
}

fn scope_order(recall: &RecallScope, resolved: &Resolved) -> Vec<Scope> {
    match recall {
        RecallScope::One(s) => vec![*s],
        RecallScope::All => {
            let mut scopes = vec![Scope::Global];
            if resolved.group.is_some() {
                scopes.push(Scope::Group);
            }
            scopes.extend([Scope::Repo, Scope::Branch, Scope::Worktree]);
            scopes
        }
    }
}

/// Serve the ranked slice for the requested scope(s).
pub fn recall(
    recall_scope: &RecallScope,
    facts: &Facts,
    resolved: &Resolved,
    topic: Option<&str>,
    budget: Option<usize>,
    include_superseded: bool,
) -> Result<Value> {
    let mut entries: Vec<Served> = Vec::new();
    let mut lines: Vec<ServedLine> = Vec::new();
    let mut served_bytes = 0usize;
    let mut entries_skipped = 0usize;
    let mut freeform_skipped = 0usize;

    for scope in scope_order(recall_scope, resolved) {
        // One-scope recall surfaces resolve errors (e.g. `group` with no
        // active group — same contract as bundle pack); `all` skips scopes
        // that cannot resolve (e.g. no group) and keeps composing.
        let target = match recall_scope {
            RecallScope::One(_) => workspace::resolve(scope, facts, resolved)?,
            RecallScope::All => match workspace::resolve(scope, facts, resolved) {
                Ok(t) => t,
                Err(_) => continue,
            },
        };
        let raw = match std::fs::read_to_string(&target.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(WhisperError::from(e)),
        };
        let scope_name = scope_name(scope);

        // Entries: filter, then rank newest-first (stable — ties keep file
        // order), then budget with whole-entry atomicity.
        let mut scoped: Vec<entry::Entry> = Vec::new();
        let mut freeform: Vec<String> = Vec::new();
        for item in entry::parse_file(&raw) {
            match item {
                Item::Entry(e) => scoped.push(e),
                Item::Line(l) if !l.trim().is_empty() => freeform.push(l),
                Item::Line(_) => {}
            }
        }
        scoped.retain(|e| include_superseded || e.superseded_by.is_none());
        if let Some(t) = topic {
            scoped.retain(|e| e.topic.as_deref() == Some(t));
        }
        scoped.sort_by(|a, b| b.ts.cmp(&a.ts));

        for e in scoped {
            let cost = e.render().len();
            match budget {
                Some(b) if served_bytes + cost > b => entries_skipped += 1,
                _ => {
                    served_bytes += cost;
                    entries.push(Served {
                        scope: scope_name,
                        entry: e,
                    });
                }
            }
        }
        for l in freeform {
            let cost = l.len() + 1;
            match budget {
                Some(b) if served_bytes + cost > b => freeform_skipped += 1,
                _ => {
                    served_bytes += cost;
                    lines.push(ServedLine {
                        scope: scope_name,
                        line: l,
                    });
                }
            }
        }
    }

    if entries.is_empty() && lines.is_empty() && entries_skipped == 0 {
        return Err(WhisperError::new("nothing to recall in this scope")
            .with_suggestion("turu init to create the layout, or append entries first"));
    }

    Ok(json!({
        "scope": match recall_scope {
            RecallScope::All => "all".to_string(),
            RecallScope::One(s) => scope_name(*s).to_string(),
        },
        "topic": topic,
        "entries": entries,
        "freeform": lines,
        "served_bytes": served_bytes,
        "budget_unused": budget.map(|b| b.saturating_sub(served_bytes)),
        "entries_skipped": entries_skipped,
        "freeform_skipped": freeform_skipped,
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
