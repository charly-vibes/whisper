//! Knowledge-verb handlers: append, recall, and the distill cycle.

use whisper::{WhisperError, distill, entry, recall, workspace};

use crate::{Cli, Commands};

use super::support::{append_scope_key, append_target, collect_text, scope_name};
use super::{Arm, Ctx};

#[allow(clippy::too_many_arguments)]
pub(crate) fn append(
    ctx: &Ctx,
    scope: workspace::Scope,
    text: Option<&str>,
    texts: &[String],
    stdin: bool,
    topic: Option<&str>,
    supersedes: Option<&str>,
    global: bool,
    private: bool,
) -> whisper::Result<Arm> {
    let text = collect_text(text, texts, stdin)?;
    reject_empty(&text)?;
    let target = append_target(scope, global, private, &ctx.facts, &ctx.resolved)?;
    let ts = entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref())?;
    let report = workspace::append_entry(
        &target,
        &append_scope_key(scope, &target, &ctx.facts, &ctx.resolved),
        &text,
        topic,
        supersedes,
        &ts,
    )?;
    Ok((
        append_data(&target, &report),
        append_warnings(&text),
        append_hint(&report),
    ))
}

/// The append envelope: scope, path, and the write report fields.
fn append_data(target: &workspace::Target, report: &workspace::AppendReport) -> serde_json::Value {
    serde_json::json!({
        "scope": target.scope,
        "path": target.path,
        "appended_bytes": report.bytes,
        "id": report.id,
        "duplicate": report.duplicate,
        "superseded": report.superseded,
    })
}

/// Advisory incident-log lint (whisper-4xl): warn, never block.
fn append_warnings(text: &str) -> Vec<String> {
    entry::incident_log_density(text).into_iter().collect()
}

/// Duplicate writes write nothing — surface that as the next step.
fn append_hint(report: &workspace::AppendReport) -> Option<String> {
    report.duplicate.then_some(
        "entry already present (same scope + second + text) — nothing written".to_string(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn recall_cmd(
    ctx: &Ctx,
    scope: &str,
    topic: Option<String>,
    budget: Option<usize>,
    digest: bool,
    include_superseded: bool,
    record_usage: bool,
) -> whisper::Result<Arm> {
    let recall_scope = recall::parse_recall_scope(scope)?;
    let data = recall::recall(
        &recall_scope,
        &ctx.facts,
        &ctx.resolved,
        &recall::RecallQuery {
            topic,
            budget,
            include_superseded,
            digest,
            record_usage,
        },
    )?;
    let warnings = empty_scope_warnings(&data);
    Ok((
        data,
        warnings,
        Some("recall serves mechanically — when to load it is your policy".into()),
    ))
}

pub(crate) fn distill_cmd(
    ctx: &Ctx,
    scope: workspace::Scope,
    begin: bool,
    commit: bool,
    revision: Option<&str>,
) -> whisper::Result<Arm> {
    if begin == commit {
        return Err(WhisperError::new("pass exactly one of --begin or --commit"));
    }
    if begin {
        distill_begin(ctx, scope)
    } else {
        distill_commit(ctx, scope, revision)
    }
}

/// `--begin`: snapshot the scope file and hand back a revision for the
/// rewrite pass. Lints entry texts (whisper-4xl) — never the raw file,
/// where [id:…] markers would false-positive.
fn distill_begin(ctx: &Ctx, scope: workspace::Scope) -> whisper::Result<Arm> {
    let ts = entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref())?;
    let rev = distill::begin_for_scope(scope, &ctx.facts, &ctx.resolved, &ts)?;
    let warnings = snapshot_lint(&rev.snapshot_path);
    Ok((
        serde_json::json!({
            "phase": "begin",
            "revision": rev.id,
            "snapshot_path": rev.snapshot_path,
            "working_path": rev.working_path,
            "target_path": rev.target_path,
            "sha256": rev.sha256,
            "recency": rev.recency,
        }),
        warnings,
        Some(format!(
            "rewrite {} with the distilled content, then: turu distill {} --commit --revision {}",
            rev.working_path.display(),
            scope_name(scope),
            rev.id
        )),
    ))
}

/// `--commit`: verify no drift, install the distilled working file.
fn distill_commit(
    ctx: &Ctx,
    scope: workspace::Scope,
    revision: Option<&str>,
) -> whisper::Result<Arm> {
    let revision = revision.ok_or_else(|| {
        WhisperError::new("--commit requires --revision <id>")
            .with_suggestion("turu distill <scope> --begin returns the revision id")
    })?;
    let report = distill::commit_for_scope(scope, &ctx.facts, &ctx.resolved, revision)?;
    Ok((
        serde_json::json!({
            "phase": "commit",
            "revision": report.revision,
            "replaced": report.replaced,
            "snapshot_path": report.snapshot_path,
            "noop": report.noop,
        }),
        vec![],
        Some(format!(
            "snapshot retained at {} (immutable)",
            report.snapshot_path.display()
        )),
    ))
}

/// Append/recall/distill arms; None when the command is not a knowledge verb.
pub(crate) fn pick_knowledge(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    if let Some(arm) = append_recall_arm(cli, ctx)? {
        return Ok(Some(arm));
    }
    distill_arm(cli, ctx)
}

/// Append and recall arms; None for other commands.
fn append_recall_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    if let Some(arm) = append_arm(cli, ctx)? {
        return Ok(Some(arm));
    }
    recall_arm(cli, ctx)
}

/// The append arm.
fn append_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    match &cli.command {
        Commands::Append {
            scope,
            text,
            texts,
            stdin,
            topic,
            supersedes,
            global,
            private,
        } => Ok(Some(append(
            ctx,
            scope.parse()?,
            text.as_deref(),
            texts,
            *stdin,
            topic.as_deref(),
            supersedes.as_deref(),
            *global,
            *private,
        )?)),
        _ => Ok(None),
    }
}

/// The recall arm.
fn recall_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    match &cli.command {
        Commands::Recall {
            scope,
            topic,
            budget,
            digest,
            include_superseded,
            no_usage,
        } => Ok(Some(recall_cmd(
            ctx,
            scope,
            topic.clone(),
            *budget,
            *digest,
            *include_superseded,
            !*no_usage,
        )?)),
        _ => Ok(None),
    }
}

/// The distill cycle arm (exactly one of --begin/--commit).
fn distill_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    match &cli.command {
        Commands::Distill {
            scope,
            begin,
            commit,
            revision,
        } => Ok(Some(distill_cmd(
            ctx,
            scope.parse()?,
            *begin,
            *commit,
            revision.as_deref(),
        )?)),
        _ => Ok(None),
    }
}

/// Empty scope (whisper-2c1): ok:true + advisory warning, never a hard
/// failure that breaks && chains or set -e.
fn empty_scope_warnings(data: &serde_json::Value) -> Vec<String> {
    if data.get("empty") == Some(&serde_json::Value::Bool(true)) {
        vec!["scope is empty — nothing to recall; append entries or check the routing block".into()]
    } else {
        Vec::new()
    }
}

/// Lint the snapshot's entry texts (whisper-4xl) — never the raw file,
/// where [id:…] markers would false-positive every well-formed file.
fn snapshot_lint(snapshot_path: &std::path::Path) -> Vec<String> {
    let snapshot = std::fs::read_to_string(snapshot_path).unwrap_or_default();
    entry::parse_file(&snapshot)
        .into_iter()
        .filter_map(|item| match item {
            entry::Item::Entry(e) => {
                entry::incident_log_density(&e.text).map(|a| format!("{}: {}", &e.id[..12], a))
            }
            _ => None,
        })
        .collect()
}

/// Loud failure when append gets no payload.
fn reject_empty(text: &str) -> whisper::Result<()> {
    if text.trim().is_empty() {
        return Err(WhisperError::new("nothing to append")
            .with_suggestion("pass --text \"...\", a positional text, or pipe stdin"));
    }
    Ok(())
}
