//! `whisper` — deterministic knowledge workspace management for AI agents.
//!
//! The mechanical half of the incitaciones whisper skill as a binary.

use std::cell::Cell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Subcommand};
use genesis::guide::{CliFormat, CliVerbosity, Output, OutputFormat, Verbosity};
use whisper::{CLI_VERSION, WhisperError, config, doctor, skill_pack, workspace};

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
    },
    /// Create the workspace layout for this checkout (never overwrites).
    Init,
    /// One envelope with every relevant path and existence flag.
    Status,
    /// Detect legacy key variants, undefined groups, and missing files.
    Check,
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
        } => {
            let scope: workspace::Scope = scope.parse()?;
            let text = collect_text(texts, *stdin)?;
            if text.trim().is_empty() {
                return Err(WhisperError::new("nothing to append")
                    .with_suggestion("pass --text \"...\" or --stdin"));
            }
            let target = workspace::resolve(scope, &facts, &resolved)?;
            target.append(&text).map_err(WhisperError::from)?;
            let data = serde_json::json!({
                "scope": target.scope,
                "path": target.path,
                "appended_bytes": text.len(),
            });
            (data, vec![], None)
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
