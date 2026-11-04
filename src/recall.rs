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

use crate::Result;
use crate::config::Resolved;
use crate::entry::{self, Item};
use crate::serving::{Acc, Ctx, scope_name};
use crate::workspace::{self, Facts, Scope};

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
    let one = |scope: Scope| -> Result<Vec<Store>> {
        match scope {
            Scope::Repo | Scope::Branch => {
                // Checkout layer in store-major order: the committed
                // repo-local file, then the private zone, then the
                // machine-local store as the fallback layer.
                let checkout = checkout_stores(scope, facts);
                Ok(checkout
                    .into_iter()
                    .chain(std::iter::once(Store {
                        scope,
                        path: Some(store_path(scope, facts, resolved)?),
                    }))
                    .collect())
            }
            _ => Ok(vec![Store {
                scope,
                path: Some(store_path(scope, facts, resolved)?),
            }]),
        }
    };
    match recall {
        RecallScope::One(s) => one(*s),
        RecallScope::All => all_scope_stores(facts, resolved, &one),
    }
}

/// Machine-local store destination for a scope (the pre-decision-A home
/// and the --global escape hatch target).
fn store_path(scope: Scope, facts: &Facts, resolved: &Resolved) -> Result<std::path::PathBuf> {
    let mut f = facts.clone();
    f.repo_local_root = None;
    Ok(workspace::resolve(scope, &f, resolved)?.path)
}

/// Stores for `all`: global, then the active group, then each checkout
/// scope. Scopes that cannot resolve (e.g. no group) are skipped silently.
fn all_scope_stores(
    facts: &Facts,
    resolved: &Resolved,
    one: &impl Fn(Scope) -> Result<Vec<Store>>,
) -> Result<Vec<Store>> {
    let group = if resolved.group.is_some() {
        vec![Store {
            scope: Scope::Group,
            path: store_path(Scope::Group, facts, resolved).ok(),
        }]
    } else {
        Vec::new()
    };
    Ok(std::iter::once(Store {
        scope: Scope::Global,
        path: store_path(Scope::Global, facts, resolved).ok(),
    })
    .chain(group)
    .chain(
        [Scope::Repo, Scope::Branch, Scope::Worktree]
            .into_iter()
            .map(one)
            .collect::<Result<Vec<Vec<Store>>>>()?
            .into_iter()
            .flatten(),
    )
    .collect::<Vec<Store>>())
}

/// The committed repo-local scope file for a checkout layer.
fn repo_file(scope: Scope, facts: &Facts) -> std::path::PathBuf {
    let root = facts.repo_local_root.as_ref().expect("checked by caller");
    match scope {
        Scope::Repo => root.join("env.md"),
        Scope::Branch => root
            .join("branches")
            .join(&facts.branch_slug)
            .join("notes.md"),
        _ => unreachable!(),
    }
}

/// The private zone scope file for a checkout layer (read-included,
/// publish-excluded — a recall never publishes).
fn private_file(scope: Scope, facts: &Facts) -> std::path::PathBuf {
    let root = facts.repo_local_root.as_ref().expect("checked by caller");
    match scope {
        Scope::Repo => root.join("private").join("env.md"),
        Scope::Branch => root
            .join("private")
            .join("branches")
            .join(&facts.branch_slug)
            .join("notes.md"),
        _ => unreachable!(),
    }
}

/// Checkout-layer stores for a repo/branch scope, in store-major order
/// (empty when the checkout has no repo-local root).
fn checkout_stores(scope: Scope, facts: &Facts) -> Vec<Store> {
    if facts.repo_local_root.is_none() {
        return Vec::new();
    }
    vec![
        Store {
            scope,
            path: Some(repo_file(scope, facts)),
        },
        Store {
            scope,
            path: Some(private_file(scope, facts)),
        },
    ]
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

/// Walk the composing stores in store-major order and serve their ranked
/// contents into `acc` (see `serve_one_store` for per-store ordering and
/// dedup rules).
fn serve_stores(
    recall_scope: &RecallScope,
    facts: &Facts,
    resolved: &Resolved,
    query: &RecallQuery,
    acc: &mut Acc,
    budget: Option<usize>,
) -> Result<()> {
    for store in stores_for(recall_scope, facts, resolved)? {
        serve_one_store(store, query, acc, budget)?;
    }
    Ok(())
}

/// Serve one store's scope file into `acc` (missing files serve nothing).
fn serve_one_store(
    store: Store,
    query: &RecallQuery,
    acc: &mut Acc,
    budget: Option<usize>,
) -> Result<()> {
    let Some(path) = store.path else {
        return Ok(());
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let (scoped, freeform) = rank_store(&raw, query);
    let scope_name = scope_name(store.scope);
    let ctx = Ctx {
        query,
        path: &path,
        budget,
    };
    for e in scoped {
        acc.serve_entry(scope_name, e, &ctx);
    }
    for l in freeform {
        acc.serve_freeform(scope_name, l, &ctx);
    }
    Ok(())
}
/// Parse one scope file into (entries, freeform lines): entries filtered
/// by superseded/topic, ranked newest-first (stable — ties keep file order).
fn rank_store(raw: &str, query: &RecallQuery) -> (Vec<entry::Entry>, Vec<String>) {
    let freeform: Vec<String> = entry::parse_file(raw)
        .into_iter()
        .filter_map(|item| match item {
            Item::Line(l) if !l.trim().is_empty() => Some(l),
            _ => None,
        })
        .collect();
    let mut scoped: Vec<entry::Entry> = entry::parse_file(raw)
        .into_iter()
        .filter_map(|item| match item {
            Item::Entry(e) => Some(e),
            Item::Line(_) => None,
        })
        .filter(|e| query.include_superseded || e.superseded_by.is_none())
        .filter(|e| query.topic.is_none() || e.topic.as_deref() == query.topic.as_deref())
        .collect();
    scoped.sort_by(|a, b| b.ts.cmp(&a.ts));
    (scoped, freeform)
}

/// Serve the ranked slice for the requested scope(s). When `record_usage`
/// is true, every entry actually served gets one usage record appended to
/// its scope's sidecar (usage only counts what recall served).
///
/// Digest budget: explicit --budget wins; otherwise the digest default
/// caps the envelope under the harness truncation line. Usage sidecars:
/// one append per touched scope file (best-effort).
pub fn recall(
    recall_scope: &RecallScope,
    facts: &Facts,
    resolved: &Resolved,
    query: &RecallQuery,
) -> Result<Value> {
    let mut acc = Acc::new();
    let budget = if query.digest {
        query.budget.or(Some(DIGEST_DEFAULT_BUDGET))
    } else {
        query.budget
    };
    let now = entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref())?;
    serve_stores(recall_scope, facts, resolved, query, &mut acc, budget)?;

    let recorded = acc.record_usage(&now);

    // Empty-scope semantics (whisper-2c1): an empty scope is an empty
    // answer, not a hard failure — ok:true with empty data and exit 0, so
    // && chains and set -e survive it. The emptiness rides in the envelope
    // ("empty": true) and main.rs surfaces it as an advisory warning.
    Ok(render(
        recall_scope,
        query,
        &acc,
        budget,
        recorded,
        acc.nothing_served(),
    ))
}

/// Mode-specific envelope payload: digest head lines versus full entries.
fn insert_payload(m: &mut serde_json::Map<String, Value>, query: &RecallQuery, acc: &Acc) {
    if query.digest {
        m.insert("digest".into(), json!(true));
        m.insert("lines".into(), json!(acc.digest_lines));
    } else {
        m.insert("entries".into(), json!(acc.entries));
        m.insert("freeform".into(), json!(acc.lines));
    }
}

/// Human label for the recall scope in the envelope.
fn scope_label(recall_scope: &RecallScope) -> String {
    match recall_scope {
        RecallScope::All => "all".to_string(),
        RecallScope::One(s) => scope_name(*s).to_string(),
    }
}

/// Render the recall envelope: shared shape, with digest-mode lines versus
/// full entries/freeform as the mode-specific payload.
fn render(
    recall_scope: &RecallScope,
    query: &RecallQuery,
    acc: &Acc,
    budget: Option<usize>,
    recorded: usize,
    nothing_served: bool,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("scope".into(), json!(scope_label(recall_scope)));
    m.insert("topic".into(), json!(query.topic));
    insert_payload(&mut m, query, acc);
    m.insert("empty".into(), json!(nothing_served));
    m.insert("served_bytes".into(), json!(acc.served_bytes));
    m.insert(
        "budget_unused".into(),
        json!(budget.map(|b| b.saturating_sub(acc.served_bytes))),
    );
    m.insert("entries_skipped".into(), json!(acc.entries_skipped));
    m.insert("freeform_skipped".into(), json!(acc.freeform_skipped));
    m.insert(
        "usage_recorded".into(),
        json!(query.record_usage.then_some(recorded)),
    );
    Value::Object(m)
}
