//! `whisper` — deterministic knowledge workspace management for AI agents.
//!
//! The mechanical half of the incitaciones whisper skill as a binary.

use std::cell::Cell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Subcommand};
use genesis::guide::{CliFormat, CliVerbosity, Output, OutputFormat, Verbosity};
use whisper::{CLI_VERSION, WhisperError, config, doctor, entry, recall, skill_pack, workspace};

thread_local! {
    static FORMAT: Cell<OutputFormat> = const { Cell::new(OutputFormat::Human) };
    static VERBOSITY: Cell<Verbosity> = const { Cell::new(Verbosity::Normal) };
}

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version = CLI_VERSION,
    about = "Deterministic knowledge workspace management for AI agents",
    after_help = genesis::guide::Verbosity::help_footer()
)]
struct Cli {
    #[command(flatten)]
    verbose: CliVerbosity,

    #[command(flatten)]
    format: CliFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Canonical repo key, branch slug, and worktree slot (pure determinism).
    Key,
    /// Resolve the exact destination path for a knowledge scope.
    Resolve {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
    },
    /// Append text to a scope's file (extend, don't duplicate).
    Append {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
        /// Text to append (repeatable).
        #[arg(long = "text")]
        texts: Vec<String>,
        /// Read the text from stdin instead of --text.
        #[arg(long)]
        stdin: bool,
        /// Optional topic key (bare word, no whitespace/parens/#).
        #[arg(long)]
        topic: Option<String>,
        /// Mark the entry with this id as superseded by the new entry.
        #[arg(long = "supersedes")]
        supersedes: Option<String>,
    },
    /// Serve a ranked, budget-bounded slice of a scope's entries.
    Recall {
        /// Scope: global | repo | branch | worktree | group | all
        #[arg()]
        scope: String,
        /// Only entries with this topic key.
        #[arg(long)]
        topic: Option<String>,
        /// Byte budget; whole entries only (never truncated).
        #[arg(long)]
        budget: Option<usize>,
        /// Include superseded entries.
        #[arg(long)]
        include_superseded: bool,
    },
    /// Create the workspace layout for this checkout (never overwrites).
    Init,
    /// One envelope with every relevant path and existence flag.
    Status,
    /// Detect legacy key variants, undefined groups, and missing files.
    Check,
    /// Migrate legacy repo-key directories into the canonical key.
    Consolidate,
    /// Deep workspace diagnostics (layout, groups, managed block, legacy keys).
    Doctor,
    /// Inject/refresh the turu managed block in the agent-facing file.
    Sync {
        /// Target file (default: AGENTS.md at the repo root).
        #[arg(long)]
        file: Option<String>,
    },
    /// Install the first-party whisper skill pack (managed by turu).
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
}

#[derive(Subcommand)]
enum SkillCommand {
    /// Install the pack (default: <repo-root>/.turu/skills).
    Install {
        /// Destination dir. Common alternatives: ~/.claude/skills,
        /// ~/.config/agents/skills.
        dir: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    FORMAT.with(|f| f.set(cli.format.format()));
    VERBOSITY.with(|v| v.set(cli.verbose.verbosity()));

    let code = run(cli);
    exit(code);
}

fn run(cli: Cli) -> i32 {
    match dispatch(&cli) {
        Ok(output) => emit_ok(&output),
        Err(err) => {
            let mut out = Output::<String>::failure(err.message.clone());
            if let Some(s) = &err.suggestion {
                out = out.with_next_step(s);
            }
            emit_err(&out);
            1
        }
    }
}

fn emit_ok<T: serde::Serialize + std::fmt::Debug>(output: &Output<T>) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let format = FORMAT.with(|f| f.get());
    let verbosity = VERBOSITY.with(|v| v.get());
    match output.emit(CLI_VERSION, format, verbosity, &mut stdout, &mut stderr) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(stderr, "whisper: write error: {e}");
            1
        }
    }
}

fn emit_err(output: &Output<String>) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let format = FORMAT.with(|f| f.get());
    let verbosity = VERBOSITY.with(|v| v.get());
    match output.emit(CLI_VERSION, format, verbosity, &mut stdout, &mut stderr) {
        Ok(()) => 1,
        Err(_) => 1,
    }
}

fn dispatch(cli: &Cli) -> whisper::Result<Output<serde_json::Value>> {
    let cwd = std::env::current_dir().map_err(WhisperError::from)?;
    let facts = workspace::collect_facts(&cwd);
    let resolved = config::load(&cwd, &facts.repo_key)?;

    let out: (serde_json::Value, Vec<String>, Option<String>) = match &cli.command {
        Commands::Key => {
            let data = serde_json::json!({
                "repo_key": facts.repo_key,
                "branch_slug": facts.branch_slug,
                "worktree_slot": facts.worktree_slot,
                "workspace_root": resolved.workspace_root,
                "group": resolved.group.as_ref().map(|(n, _)| n.clone()),
            });
            (
                data,
                vec![],
                Some("whisper resolve <scope> to get an exact write destination".into()),
            )
        }
        Commands::Resolve { scope } => {
            let scope: workspace::Scope = scope.parse()?;
            let target = workspace::resolve(scope, &facts, &resolved)?;
            let data = serde_json::json!({
                "scope": target.scope,
                "path": target.path,
                "exists": target.path.exists(),
            });
            (
                data,
                vec![],
                Some(format!(
                    "whisper append {} --text \"...\"",
                    scope_name(target.scope)
                )),
            )
        }
        Commands::Append {
            scope,
            texts,
            stdin,
            topic,
            supersedes,
        } => {
            let scope: workspace::Scope = scope.parse()?;
            let text = collect_text(texts, *stdin)?;
            if text.trim().is_empty() {
                return Err(WhisperError::new("nothing to append")
                    .with_suggestion("pass --text \"...\" or --stdin"));
            }
            let target = workspace::resolve(scope, &facts, &resolved)?;
            let ts = entry::parse_or_now(std::env::var("TURU_NOW").ok().as_deref())?;
            let report = workspace::append_entry(
                &target,
                &workspace::scope_key(scope, &facts, &resolved),
                &text,
                topic.as_deref(),
                supersedes.as_deref(),
                &ts,
            )?;
            let data = serde_json::json!({
                "scope": target.scope,
                "path": target.path,
                "appended_bytes": if report.duplicate { 0 } else { text.len() },
                "id": report.id,
                "duplicate": report.duplicate,
                "superseded": report.superseded,
            });
            let hint = if report.duplicate {
                Some(
                    "entry already present (same scope + second + text) — nothing written"
                        .to_string(),
                )
            } else {
                None
            };
            (data, vec![], hint)
        }
        Commands::Recall {
            scope,
            topic,
            budget,
            include_superseded,
        } => {
            let recall_scope = recall::parse_recall_scope(scope)?;
            let data = recall::recall(
                &recall_scope,
                &facts,
                &resolved,
                topic.as_deref(),
                *budget,
                *include_superseded,
            )?;
            (
                data,
                vec![],
                Some("recall serves mechanically — when to load it is your policy".into()),
            )
        }
        Commands::Init => {
            let report = workspace::init(&facts, &resolved)?;
            let data = serde_json::json!({
                "workspace_root": report.workspace_root,
                "group": report.group,
                "created": report.created,
                "existing": report.existing,
            });
            (
                data,
                vec![],
                Some("whisper status to see the full layout".to_string()),
            )
        }
        Commands::Status => {
            let data = serde_json::json!({
                "workspace_root": resolved.workspace_root,
                "group": resolved.group.as_ref().map(|(n, _)| n.clone()),
                "repo_key": facts.repo_key,
                "branch_slug": facts.branch_slug,
                "worktree_slot": facts.worktree_slot,
                "paths": paths_for(&facts, &resolved)?,
            });
            (
                data,
                vec![],
                Some("whisper check to validate the workspace".to_string()),
            )
        }
        Commands::Check => {
            let mut warnings = Vec::new();
            let paths = paths_for(&facts, &resolved)?;
            let variants = workspace::legacy_variants(&facts, &resolved);
            if !variants.is_empty() {
                warnings.push(format!(
                    "legacy repo-key variants found under repos/: [{}] — route new knowledge into the canonical key '{}'",
                    variants.join(", "),
                    facts.repo_key
                ));
            }
            if !resolved.workspace_root.join("rules.md").exists() {
                warnings.push(format!(
                    "global rules file missing at {}",
                    resolved.workspace_root.join("rules.md").display()
                ));
            }
            let data = serde_json::json!({
                "workspace_root": resolved.workspace_root,
                "repo_key": facts.repo_key,
                "legacy_variants": variants,
                "paths": paths,
            });
            let hint = if warnings.is_empty() {
                Some("workspace looks consistent".to_string())
            } else {
                Some("whisper init to create missing files".to_string())
            };
            (data, warnings, hint)
        }
        Commands::Consolidate => {
            let report = workspace::consolidate(&facts, &resolved)?;
            let data = serde_json::json!({
                "canonical_dir": report.canonical_dir,
                "variants": report.variants,
                "moved": report.moved,
                "merged": report.merged,
            });
            (
                data,
                vec![],
                Some("turu doctor — the legacy-keys check must pass".to_string()),
            )
        }
        Commands::Doctor => {
            let report = doctor::run_checks(&facts, &resolved, &cwd);
            let hint = if report.is_healthy() {
                "workspace is healthy".to_string()
            } else {
                "apply the listed fix commands, or run `turu init`".to_string()
            };
            let data =
                serde_json::to_value(&report).map_err(|e| WhisperError::new(e.to_string()))?;
            (data, vec![], Some(hint))
        }
        Commands::Sync { file } => {
            let (target, outcome) =
                workspace::agents_sync(&cwd, file.as_deref(), &facts, &resolved)?;
            let data = serde_json::json!({
                "target": target,
                "outcome": outcome,
            });
            (
                data,
                vec![],
                Some(format!(
                    "`{}` now carries the deterministic routing map ({outcome})",
                    target.display()
                )),
            )
        }
        Commands::Skill {
            command: SkillCommand::Install { dir },
        } => {
            let root = match dir {
                Some(d) => PathBuf::from(d),
                None => workspace::repo_root(&cwd).join(".turu").join("skills"),
            };
            let pack_dir = root.join("whisper");
            let files = skill_pack::generate_pack("whisper").map_err(WhisperError::new)?;
            for (rel, content) in &files {
                let dest = pack_dir.join(rel);
                std::fs::create_dir_all(dest.parent().unwrap()).map_err(WhisperError::from)?;
                std::fs::write(&dest, content).map_err(WhisperError::from)?;
            }
            let data = serde_json::json!({
                "pack": "whisper",
                "target": pack_dir,
                "files": files.len(),
                "hash": skill_pack::pack_content_hash(&files),
            });
            (
                data,
                vec![],
                Some(format!(
                    "pack installed at `{}` — `turu doctor` reports staleness",
                    pack_dir.display()
                )),
            )
        }
    };

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

fn scope_name(scope: workspace::Scope) -> &'static str {
    match scope {
        workspace::Scope::Global => "global",
        workspace::Scope::Repo => "repo",
        workspace::Scope::Branch => "branch",
        workspace::Scope::Worktree => "worktree",
        workspace::Scope::Group => "group",
    }
}

fn paths_for(
    facts: &workspace::Facts,
    resolved: &config::Resolved,
) -> whisper::Result<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    for scope in [
        workspace::Scope::Global,
        workspace::Scope::Repo,
        workspace::Scope::Branch,
        workspace::Scope::Worktree,
        workspace::Scope::Group,
    ] {
        // Group scope may legitimately be inactive; record it as null.
        match workspace::resolve(scope, facts, resolved) {
            Ok(t) => {
                map.insert(scope_name(scope).to_string(), serde_json::json!(t.path));
            }
            Err(_) => {
                map.insert(scope_name(scope).to_string(), serde_json::Value::Null);
            }
        }
    }
    Ok(map)
}

fn collect_text(texts: &[String], use_stdin: bool) -> whisper::Result<String> {
    if use_stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(WhisperError::from)?;
        return Ok(buf);
    }
    Ok(texts.join("\n"))
}
