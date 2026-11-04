//! Command dispatch: one handler per CLI verb, each returning the
//! (data, warnings, next-step-hint) triple the envelope is built from.

mod admin;
mod knowledge;
mod support;

use std::path::PathBuf;

use genesis::guide::Output;
use whisper::{WhisperError, bundle, config, workspace};

use crate::{BundleCommand, Cli, Commands, SkillCommand};
use support::{append_target, paths_for, scope_name};

/// One command handler's raw output: envelope data, advisory warnings, and
/// the next-step hint.
pub(crate) type Arm = (serde_json::Value, Vec<String>, Option<String>);

/// Per-invocation context handed to every handler.
pub(crate) struct Ctx {
    facts: workspace::Facts,
    resolved: config::Resolved,
    cwd: PathBuf,
}

pub(crate) fn dispatch(cli: &Cli) -> whisper::Result<Output<serde_json::Value>> {
    let ctx = Ctx::collect()?;
    let out = pick(cli, &ctx)?;
    let (data, warnings, hint) = out;
    let mut output = Output::success(data).with_verbosity(0);
    for w in warnings {
        output = output.with_warning(w);
    }
    if let Some(h) = hint {
        output = output.with_next_step(h);
    }
    Ok(output)
}

impl Ctx {
    fn collect() -> whisper::Result<Self> {
        let cwd = std::env::current_dir().map_err(WhisperError::from)?;
        let facts = workspace::collect_facts(&cwd);
        let resolved = config::load(&cwd, &facts.repo_key)?;
        Ok(Self {
            facts,
            resolved,
            cwd,
        })
    }
}

/// Core knowledge-verb arms; admin verbs fall through to `admin_arm`.
fn pick(cli: &Cli, ctx: &Ctx) -> whisper::Result<Arm> {
    if let Some(out) = knowledge::pick_knowledge(cli, ctx)? {
        return Ok(out);
    }
    match &cli.command {
        Commands::Key => key(ctx),
        Commands::Resolve {
            scope,
            global,
            private,
        } => resolve(ctx, scope.parse()?, *global, *private),
        Commands::Init => init(ctx),
        Commands::Status => status(ctx),
        Commands::Check => check(ctx),
        Commands::Consolidate => consolidate(ctx),
        Commands::Bundle { command } => bundle_cmd(ctx, command),
        _ => init_arm(cli, ctx)?.map_or_else(|| admin_arm(cli, ctx), Ok),
    }
}

/// Workspace-shape verbs: init, status, check, consolidate.
fn init_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Option<Arm>> {
    match &cli.command {
        Commands::Init => init(ctx).map(Some),
        Commands::Status => status(ctx).map(Some),
        Commands::Check => check(ctx).map(Some),
        Commands::Consolidate => consolidate(ctx).map(Some),
        _ => Ok(None),
    }
}

/// Admin verbs: doctor, sync, feedback, skill install.
fn admin_arm(cli: &Cli, ctx: &Ctx) -> whisper::Result<Arm> {
    match &cli.command {
        Commands::Doctor => admin::doctor_cmd(ctx),
        Commands::Sync { file } => admin::sync(ctx, file.as_deref()),
        Commands::Feedback {
            kind,
            from_last_error,
            dry_run,
        } => admin::feedback(kind.clone(), *dry_run, *from_last_error, ctx),
        Commands::Skill {
            command: SkillCommand::Install { dir },
        } => admin::skill_install(dir.as_deref()),
        _ => unreachable!("dispatch handled every command"),
    }
}

fn key(ctx: &Ctx) -> whisper::Result<Arm> {
    Ok((
        serde_json::json!({
            "repo_key": ctx.facts.repo_key,
            "branch_slug": ctx.facts.branch_slug,
            "worktree_slot": ctx.facts.worktree_slot,
            "workspace_root": ctx.resolved.workspace_root,
            "group": ctx.resolved.group.as_ref().map(|(n, _)| n.clone()),
        }),
        vec![],
        Some("turu resolve <scope> to get an exact write destination".into()),
    ))
}

fn resolve(
    ctx: &Ctx,
    scope: workspace::Scope,
    global: bool,
    private: bool,
) -> whisper::Result<Arm> {
    let target = append_target(scope, global, private, &ctx.facts, &ctx.resolved)?;
    Ok((
        serde_json::json!({
            "scope": target.scope,
            "path": target.path,
            "exists": target.path.exists(),
        }),
        vec![],
        Some(format!(
            "turu append {} --text \"...\" (positional text or piped stdin also work)",
            scope_name(target.scope)
        )),
    ))
}

fn bundle_cmd(ctx: &Ctx, command: &BundleCommand) -> whisper::Result<Arm> {
    match command {
        BundleCommand::Pack { scope, out } => {
            let scope: workspace::Scope = scope.parse().expect("bundle: invalid scope");
            let data = bundle::pack(scope, &ctx.facts, &ctx.resolved, out.as_deref())?;
            Ok((
                data,
                vec![],
                Some("bundle is transport-agnostic — move it however you decide".into()),
            ))
        }
        BundleCommand::Unpack { file, stdin } => {
            let input = bundle::read_input(file.as_deref(), *stdin)?;
            let data = bundle::unpack(&input, &ctx.facts, &ctx.resolved)?;
            Ok((
                data,
                vec![],
                Some("merge is extend-only — existing content untouched".into()),
            ))
        }
    }
}

fn init(ctx: &Ctx) -> whisper::Result<Arm> {
    let report = workspace::init(&ctx.facts, &ctx.resolved)?;
    Ok((
        serde_json::json!({
            "workspace_root": report.workspace_root,
            "group": report.group,
            "created": report.created,
            "existing": report.existing,
            "ignore_rule_added": report.ignore_rule_added,
        }),
        vec![],
        Some("turu status to see the full layout".to_string()),
    ))
}

fn status(ctx: &Ctx) -> whisper::Result<Arm> {
    let paths = paths_for(&ctx.facts, &ctx.resolved)?;
    Ok((
        serde_json::json!({
            "workspace_root": ctx.resolved.workspace_root,
            "group": ctx.resolved.group.as_ref().map(|(n, _)| n.clone()),
            "repo_key": ctx.facts.repo_key,
            "branch_slug": ctx.facts.branch_slug,
            "worktree_slot": ctx.facts.worktree_slot,
            "paths": paths,
        }),
        vec![],
        Some("turu check to validate the workspace".to_string()),
    ))
}

fn check(ctx: &Ctx) -> whisper::Result<Arm> {
    let mut warnings = check_warnings(ctx);
    let paths = paths_for(&ctx.facts, &ctx.resolved)?;
    let variants = workspace::legacy_variants(&ctx.facts, &ctx.resolved);
    if !variants.is_empty() {
        warnings.insert(
            0,
            format!(
                "legacy repo-key variants found under repos/: [{}] — route new knowledge into the canonical key '{}'",
                variants.join(", "),
                ctx.facts.repo_key
            ),
        );
    }
    let data = serde_json::json!({
        "workspace_root": ctx.resolved.workspace_root,
        "repo_key": ctx.facts.repo_key,
        "legacy_variants": variants,
        "paths": paths,
    });
    let hint = if warnings.is_empty() {
        "workspace looks consistent".to_string()
    } else {
        "whisper init to create missing files".to_string()
    };
    Ok((data, warnings, Some(hint)))
}

/// Consistency warnings: shadowed global rules file (legacy variants are
/// reported by the caller so they can lead the list).
fn check_warnings(ctx: &Ctx) -> Vec<String> {
    let mut warnings = Vec::new();
    if !ctx.resolved.workspace_root.join("rules.md").exists() {
        warnings.push(format!(
            "global rules file missing at {}",
            ctx.resolved.workspace_root.join("rules.md").display()
        ));
    }
    if let Some(real_root) = &ctx.resolved.shadowed_global_root {
        let real = real_root.join("rules.md");
        let shadowed = ctx.resolved.workspace_root.join("rules.md");
        let diverged = shadowed.exists()
            && std::fs::read_to_string(&real).unwrap_or_default()
                != std::fs::read_to_string(&shadowed).unwrap_or_default();
        if real.exists() && (!shadowed.exists() || diverged) {
            warnings.push(format!(
                "repo-private workspace_root shadows the global rules file at {} — run `turu doctor` for detail",
                real.display()
            ));
        }
    }
    warnings
}

fn consolidate(ctx: &Ctx) -> whisper::Result<Arm> {
    let report = workspace::consolidate(&ctx.facts, &ctx.resolved)?;
    Ok((
        serde_json::json!({
            "canonical_dir": report.canonical_dir,
            "variants": report.variants,
            "moved": report.moved,
            "merged": report.merged,
        }),
        vec![],
        Some("turu doctor — the legacy-keys check must pass".to_string()),
    ))
}
