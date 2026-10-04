//! Recall serving: ranked, budget-bounded slices of scope entries.
//!
//! turu serves mechanically (filter, rank, budget); *when* to call recall
//! is the agent's policy. Whole-entry atomicity: an entry that does not fit
//! the budget is skipped in its entirety, never truncated. For repo/branch
//! scopes the composition is store-major across three layers: committed
//! repo-local file, private zone (read-included here — publication verbs,
//! not recall, own exclusion), then the machine-local store.

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

/// Default byte budget for `--digest` recalls with no explicit `--budget`.
/// Keeps the whole envelope comfortably under the ~50KB harness truncation
/// line (whisper-2c1: a fat scope's full envelope is ONE compact JSON line —
/// measured 242KB — that the agent perceives as "no output"), while still
/// serving hundreds of heads.
pub const DIGEST_DEFAULT_BUDGET: usize = 40_000;

/// Head cap: a digest line never carries more than this many bytes of
/// entry/freeform text (whole-entry atomicity in miniature — a head is a
/// pointer into the scope file, not the text itself).
const DIGEST_HEAD_CAP: usize = 160;

/// Cap text to the head budget on a char boundary.
fn cap_head(text: &str) -> String {
    if text.len() <= DIGEST_HEAD_CAP {
        return text.to_string();
    }
    let mut end = DIGEST_HEAD_CAP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// One capped head line for a served entry: ts, scope tag, topic, first
/// text line. Later lines never leak (a head points at the scope file —
/// the full text stays there).
fn digest_entry_line(scope: &str, e: &entry::Entry) -> String {
    let tag = match &e.topic {
        Some(t) => format!("[{scope}#{t}]"),
        None => format!("[{scope}]"),
    };
    let first_line = e.text.split('\n').next().unwrap_or("");
    format!("{} {} {}", e.ts, tag, cap_head(first_line))
}

/// One capped head line for a served freeform line (verbatim text, capped).
fn digest_freeform_line(scope: &str, line: &str) -> String {
    format!("[{scope}] {}", cap_head(line))
}

/// One store that participates in recall composition.
struct Store {
    /// Scope tag for served items.
    scope: Scope,
    /// The exact scope file (None: store does not apply — skip silently).
    path: Option<std::path::PathBuf>,
    // Positional note: stores arrive in store-major order — every checkout
    // (repo-local) store is listed before its machine-local store, so no
    // flag is needed here; the order itself is the contract.
}

/// The stores composing this recall, in store-major serving order. One-scope
/// recall surfaces resolve errors (e.g. `group` with no active group — same
/// contract as bundle pack); `all` skips scopes that cannot resolve (e.g. no
/// group) and keeps composing.
fn stores_for(recall: &RecallScope, facts: &Facts, resolved: &Resolved) -> Result<Vec<Store>> {
    // Machine-local store destination for a scope (the pre-decision-A home
    // and the --global escape hatch target).
    let store_path = |scope: Scope| -> Result<std::path::PathBuf> {
        let mut f = facts.clone();
        f.repo_local_root = None;
        Ok(workspace::resolve(scope, &f, resolved)?.path)
    };
    let one = |scope: Scope| -> Result<Vec<Store>> {
        match scope {
            Scope::Repo | Scope::Branch => {
                // Checkout layer in store-major order: the committed
                // repo-local file, then the private zone (read-included,
                // publish-excluded — a recall never publishes), then the
                // machine-local store as the fallback layer.
                let mut stores = Vec::new();
                if let Some(root) = &facts.repo_local_root {
                    stores.push(Store {
                        scope,
                        path: Some(match scope {
                            Scope::Repo => root.join("env.md"),
                            Scope::Branch => root
                                .join("branches")
                                .join(&facts.branch_slug)
                                .join("notes.md"),
                            _ => unreachable!(),
                        }),
                    });
                    stores.push(Store {
                        scope,
                        path: Some(match scope {
                            Scope::Repo => root.join("private").join("env.md"),
                            Scope::Branch => root
                                .join("private")
                                .join("branches")
                                .join(&facts.branch_slug)
                                .join("notes.md"),
                            _ => unreachable!(),
                        }),
                    });
                }
                stores.push(Store {
                    scope,
                    path: Some(store_path(scope)?),
                });
                Ok(stores)
            }
            _ => Ok(vec![Store {
                scope,
                path: Some(store_path(scope)?),
            }]),
        }
    };
    match recall {
        RecallScope::One(s) => one(*s),
        RecallScope::All => {
            let mut stores = vec![Store {
                scope: Scope::Global,
                path: store_path(Scope::Global).ok(),
            }];
            if resolved.group.is_some() {
                stores.push(Store {
                    scope: Scope::Group,
                    path: store_path(Scope::Group).ok(),
                });
            }
            for scope in [Scope::Repo, Scope::Branch, Scope::Worktree] {
                stores.extend(one(scope)?);
            }
            Ok(stores)
        }
    }
}

/// One recall's serving knobs, bundled to keep the call site narrow.
pub struct RecallQuery {
    /// Only entries with this topic key.
    pub topic: Option<String>,
    /// Byte budget; whole entries only (never truncated).
    pub budget: Option<usize>,
    /// Include superseded entries.
    pub include_superseded: bool,
    /// Serve capped one-line heads (entries + freeform) instead of full
    /// entries — the fat-scope-safe default: one JSON line that stays
    /// under the ~50KB harness truncation limit.
    pub digest: bool,
    /// Record served ids to the usage sidecar.
    pub record_usage: bool,
}

/// Serve the ranked slice for the requested scope(s). When `record_usage`
/// is true, every entry actually served gets one usage record appended to
/// its scope's sidecar (filtered and budget-skipped entries are never
/// recorded — usage only counts what recall served).
pub fn recall(
    recall_scope: &RecallScope,
    facts: &Facts,
    resolved: &Resolved,
    query: &RecallQuery,
) -> Result<Value> {
    let mut entries: Vec<Served> = Vec::new();
    let mut lines: Vec<ServedLine> = Vec::new();
    let mut digest_lines: Vec<String> = Vec::new();
    let mut served_bytes = 0usize;
    // Digest budget: explicit --budget wins; otherwise the digest default
    // caps the envelope under the harness truncation line.
    let budget = if query.digest {
        query.budget.or(Some(DIGEST_DEFAULT_BUDGET))
    } else {
        query.budget
    };
    let mut entries_skipped = 0usize;
    let mut freeform_skipped = 0usize;
    let mut served_ids = std::collections::HashSet::new();
    // Entry ids served per scope file, recorded to the usage sidecar after
    // serving (skip-on-error: recall must never fail because telemetry
    // cannot write — read-only workspaces still recall).
    let mut usage_order: Vec<std::path::PathBuf> = Vec::new();
    let mut usage_batch: std::collections::HashMap<std::path::PathBuf, Vec<String>> =
        std::collections::HashMap::new();
    let now = entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref())?;

    // Stores arrive in store-major order; within a store the scope file is
    // read and ranked newest-first (stable — ties keep file order), then the
    // whole-entry byte budget applies. When a scope spans two stores, the
    // checkout's repo-local entries all serve before any workspace-root
    // entry — even a newer one — and ids are deduplicated across stores so
    // a bundled or migrated entry is never served twice.
    for store in stores_for(recall_scope, facts, resolved)? {
        let Some(path) = store.path else {
            continue;
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(WhisperError::from(e)),
        };
        let scope_name = scope_name(store.scope);

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
        scoped.retain(|e| query.include_superseded || e.superseded_by.is_none());
        if let Some(t) = &query.topic {
            scoped.retain(|e| e.topic.as_deref() == Some(t));
        }
        scoped.sort_by(|a, b| b.ts.cmp(&a.ts));

        for e in scoped {
            if !served_ids.insert(e.id.clone()) {
                continue; // same entry reached through another store
            }
            let cost = match query.digest {
                true => digest_entry_line(scope_name, &e).len() + 1,
                false => e.render().len(),
            };
            match budget {
                Some(b) if served_bytes + cost > b => entries_skipped += 1,
                _ => {
                    served_bytes += cost;
                    if query.digest {
                        digest_lines.push(digest_entry_line(scope_name, &e));
                    }
                    if query.record_usage {
                        usage_batch
                            .entry(path.clone())
                            .or_insert_with(|| {
                                usage_order.push(path.clone());
                                Vec::new()
                            })
                            .push(e.id.clone());
                    }
                    entries.push(Served {
                        scope: scope_name,
                        entry: e,
                    });
                }
            }
        }
        for l in freeform {
            let cost = match query.digest {
                true => digest_freeform_line(scope_name, &l).len() + 1,
                false => l.len() + 1,
            };
            match budget {
                Some(b) if served_bytes + cost > b => freeform_skipped += 1,
                _ => {
                    served_bytes += cost;
                    if query.digest {
                        digest_lines.push(digest_freeform_line(scope_name, &l));
                    } else {
                        lines.push(ServedLine {
                            scope: scope_name,
                            line: l,
                        });
                    }
                }
            }
        }
    }

    // One sidecar per scope file touched (O_APPEND); best-effort by design.
    let mut recorded = 0usize;
    for path in &usage_order {
        if let Some(ids) = usage_batch.get_mut(path) {
            recorded += crate::usage::append(path, ids, &now).unwrap_or(0);
        }
    }

    // Empty-scope semantics (whisper-2c1): an empty scope is an empty
    // answer, not a hard failure — ok:true with empty data and exit 0, so
    // && chains and set -e survive it. The emptiness rides in the envelope
    // ("empty": true) and main.rs surfaces it as an advisory warning.
    let nothing_served =
        entries.is_empty() && lines.is_empty() && digest_lines.is_empty() && entries_skipped == 0;

    if query.digest {
        return Ok(json!({
            "scope": match recall_scope {
                RecallScope::All => "all".to_string(),
                RecallScope::One(s) => scope_name(*s).to_string(),
            },
            "topic": query.topic,
            "digest": true,
            "empty": nothing_served,
            "lines": digest_lines,
            "served_bytes": served_bytes,
            "budget_unused": budget.map(|b| b.saturating_sub(served_bytes)),
            "entries_skipped": entries_skipped,
            "freeform_skipped": freeform_skipped,
            "usage_recorded": query.record_usage.then_some(recorded),
        }));
    }

    Ok(json!({
        "scope": match recall_scope {
            RecallScope::All => "all".to_string(),
            RecallScope::One(s) => scope_name(*s).to_string(),
        },
        "topic": query.topic,
        "empty": nothing_served,
        "entries": entries,
        "freeform": lines,
        "served_bytes": served_bytes,
        "budget_unused": budget.map(|b| b.saturating_sub(served_bytes)),
        "entries_skipped": entries_skipped,
        "freeform_skipped": freeform_skipped,
        "usage_recorded": query.record_usage.then_some(recorded),
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
