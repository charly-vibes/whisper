//! Digest-line rendering and the serving accumulator shared by `recall`.
//! Split out of `recall.rs` to keep file and function sizes small.

use crate::entry;
use crate::recall::{RecallQuery, Served, ServedLine};
use crate::workspace::Scope;

/// Cap text to the head budget on a char boundary.
const DIGEST_HEAD_CAP: usize = 160;

pub(super) fn cap_head(text: &str) -> String {
    if text.len() <= DIGEST_HEAD_CAP {
        return text.to_string();
    }
    let end = (0..=DIGEST_HEAD_CAP)
        .rev()
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(DIGEST_HEAD_CAP);
    text[..end].to_string()
}

/// One capped head line for a served entry: ts, scope tag, topic, first
/// text line. Later lines never leak (a head points at the scope file —
/// the full text stays there).
pub(super) fn digest_entry_line(scope: &str, e: &entry::Entry) -> String {
    let tag = match &e.topic {
        Some(t) => format!("[{scope}#{t}]"),
        None => format!("[{scope}]"),
    };
    let first_line = e.text.split('\n').next().unwrap_or("");
    format!("{} {} {}", e.ts, tag, cap_head(first_line))
}

/// One capped head line for a served freeform line (verbatim text, capped).
pub(super) fn digest_freeform_line(scope: &str, line: &str) -> String {
    format!("[{scope}] {}", cap_head(line))
}

/// Per-store serving context: the query knobs, the scope file being served,
/// and the active byte budget.
pub(super) struct Ctx<'a> {
    pub(super) query: &'a RecallQuery,
    pub(super) path: &'a std::path::Path,
    pub(super) budget: Option<usize>,
}

/// Accumulated serving state for one recall run. Mutable internals are
/// confined to `&mut self` methods so the top-level `recall` stays a
/// composition of store loops plus one JSON render.
pub(super) struct Acc {
    pub(super) entries: Vec<Served>,
    pub(super) lines: Vec<ServedLine>,
    pub(super) digest_lines: Vec<String>,
    pub(super) served_bytes: usize,
    pub(super) entries_skipped: usize,
    pub(super) freeform_skipped: usize,
    pub(super) served_ids: std::collections::HashSet<String>,
    /// Entry ids per scope file, in first-touched order (usage sidecars).
    pub(super) usage_order: Vec<std::path::PathBuf>,
    pub(super) usage_batch: std::collections::HashMap<std::path::PathBuf, Vec<String>>,
}

impl Acc {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
            lines: Vec::new(),
            digest_lines: Vec::new(),
            served_bytes: 0,
            entries_skipped: 0,
            freeform_skipped: 0,
            served_ids: std::collections::HashSet::new(),
            usage_order: Vec::new(),
            usage_batch: std::collections::HashMap::new(),
        }
    }

    /// Budget test + serve for one entry (whole-entry atomicity).
    pub(super) fn serve_entry(&mut self, scope_name: &'static str, e: entry::Entry, ctx: &Ctx) {
        if !self.served_ids.insert(e.id.clone()) {
            return; // same entry reached through another store
        }
        let cost = match ctx.query.digest {
            true => digest_entry_line(scope_name, &e).len() + 1,
            false => e.render().len(),
        };
        if self.over_budget(ctx.budget, cost) {
            self.entries_skipped += 1;
            return;
        }
        self.served_bytes += cost;
        if ctx.query.digest {
            self.digest_lines.push(digest_entry_line(scope_name, &e));
        }
        if ctx.query.record_usage {
            self.record_served_id(ctx.path, &e.id);
        }
        self.entries.push(Served {
            scope: scope_name,
            entry: e,
        });
    }

    /// Whole-line atomicity: never serve a partial item over the budget.
    fn over_budget(&self, budget: Option<usize>, cost: usize) -> bool {
        budget.is_some_and(|b| self.served_bytes + cost > b)
    }

    /// Record one served entry id for its scope file's usage sidecar.
    fn record_served_id(&mut self, path: &std::path::Path, id: &str) {
        self.usage_batch
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                self.usage_order.push(path.to_path_buf());
                Vec::new()
            })
            .push(id.to_string());
    }

    /// Budget test + serve for one freeform line (whole-line atomicity).
    pub(super) fn serve_freeform(&mut self, scope_name: &'static str, l: String, ctx: &Ctx) {
        let cost = match ctx.query.digest {
            true => digest_freeform_line(scope_name, &l).len() + 1,
            false => l.len() + 1,
        };
        if self.over_budget(ctx.budget, cost) {
            self.freeform_skipped += 1;
            return;
        }
        self.served_bytes += cost;
        if ctx.query.digest {
            self.digest_lines.push(digest_freeform_line(scope_name, &l));
        } else {
            self.lines.push(ServedLine {
                scope: scope_name,
                line: l,
            });
        }
    }

    /// Append one usage record per served id to each touched scope file's
    /// sidecar (O_APPEND; best-effort by design — recall must never fail
    /// because telemetry cannot write; read-only workspaces still recall).
    pub(super) fn record_usage(&self, now: &str) -> usize {
        self.usage_order
            .iter()
            .filter_map(|path| self.usage_batch.get(path).map(|ids| (path, ids)))
            .map(|(path, ids)| crate::usage::append(path, ids, now).unwrap_or(0))
            .sum()
    }

    pub(super) fn nothing_served(&self) -> bool {
        self.entries.is_empty()
            && self.lines.is_empty()
            && self.digest_lines.is_empty()
            && self.entries_skipped == 0
    }
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
