//! Admin verbs: doctor, sync, feedback, skill install.

use std::path::PathBuf;

use whisper::{CLI_VERSION, WhisperError, doctor, skill_pack, workspace};

use super::{Arm, Ctx};

pub(crate) fn doctor_cmd(ctx: &Ctx) -> whisper::Result<Arm> {
    let report = doctor::run_checks(&ctx.facts, &ctx.resolved, &ctx.cwd);
    // Advisory leak-shape findings surface in warnings[] too (3.2c).
    let warnings = doctor::leak_warnings(&ctx.facts, &ctx.resolved);
    let hint = if report.is_healthy() {
        "workspace is healthy".to_string()
    } else {
        "apply the listed fix commands, or run `turu init`".to_string()
    };
    let data = serde_json::to_value(&report).map_err(|e| WhisperError::new(e.to_string()))?;
    Ok((data, warnings, Some(hint)))
}

/// Checkout self-protection (add-repo-private-scope 3.1): sync ensures the
/// private-zone ignore rule alongside the routing map.
pub(crate) fn sync(ctx: &Ctx, file: Option<&str>) -> whisper::Result<Arm> {
    let ignore_rule_added = workspace::ensure_private_ignored(&ctx.facts)?;
    let (target, outcome) = workspace::agents_sync(&ctx.cwd, file, &ctx.facts, &ctx.resolved)?;
    Ok((
        serde_json::json!({
            "target": target,
            "outcome": outcome,
            "ignore_rule_added": ignore_rule_added,
        }),
        vec![],
        Some(format!(
            "`{}` now carries the deterministic routing map ({outcome})",
            target.display()
        )),
    ))
}

pub(crate) fn feedback(
    kind: String,
    dry_run: bool,
    from_last_error: bool,
    ctx: &Ctx,
) -> whisper::Result<Arm> {
    let args = genesis::feedback::FeedbackArgs::new(kind, dry_run, from_last_error);
    let repo = env!("CARGO_PKG_REPOSITORY")
        .trim_start_matches("https://github.com/")
        .trim_start_matches("http://github.com/")
        .to_string();
    let repo_root = workspace::repo_root(&ctx.cwd);
    let result = genesis::feedback::handle_feedback(&args, "turu", CLI_VERSION, &repo, &repo_root)
        .map_err(|msg| {
            let suggestion = if msg.starts_with("unknown kind") {
                "pass one of: bug, feature, question, chore".to_string()
            } else {
                "pipe content into stdin, use --from-last-error, or preview with `turu feedback bug --dry-run`".to_string()
            };
            WhisperError::new(msg).with_suggestion(suggestion)
        })?;
    Ok(render_feedback(result))
}

/// Fold the genesis feedback result into the (data, warnings, hint) triple.
fn render_feedback(result: genesis::feedback::gh::GhResult) -> Arm {
    match result {
        genesis::feedback::gh::GhResult::Created { url, number } => (
            serde_json::json!({ "outcome": "created", "url": url, "issue": number }),
            vec![],
            Some("issue filed — the maintainers will pick it up".to_string()),
        ),
        genesis::feedback::gh::GhResult::FallbackUrl(url) => (
            serde_json::json!({ "outcome": "fallback", "url": url }),
            vec![],
            Some("issue body printed to stderr — open the url to file it manually".to_string()),
        ),
        genesis::feedback::gh::GhResult::LocalFile(path) => (
            serde_json::json!({ "outcome": "saved-local", "file": path }),
            vec![],
            Some("network unavailable — attach this file to a new GitHub issue".to_string()),
        ),
    }
}

pub(crate) fn skill_install(dir: Option<&str>) -> whisper::Result<Arm> {
    let root = match dir {
        Some(d) => PathBuf::from(d),
        None => workspace::repo_root(&std::env::current_dir().expect("no cwd"))
            .join(".turu")
            .join("skills"),
    };
    let pack_dir = root.join("whisper");
    let files = skill_pack::generate_pack("whisper").map_err(WhisperError::new)?;
    for (rel, content) in &files {
        let dest = pack_dir.join(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(WhisperError::from)?;
        std::fs::write(&dest, content).map_err(WhisperError::from)?;
    }
    Ok((
        serde_json::json!({
            "pack": "whisper",
            "target": pack_dir,
            "files": files.len(),
            "hash": skill_pack::pack_content_hash(&files),
        }),
        vec![],
        Some(format!(
            "pack installed at `{}` — `turu doctor` reports staleness",
            pack_dir.display()
        )),
    ))
}
