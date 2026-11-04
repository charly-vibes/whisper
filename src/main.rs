//! `whisper` — deterministic knowledge workspace management for AI agents.
//!
//! The mechanical half of the incitaciones whisper skill as a binary.
//! Write-path verbs route through exactly one destination per call:
//! default (checkout's repo-local root), `--global` (machine-local store),
//! or `--private` (checkout's gitignored `.whisper/private/` zone).
//! `init` and `sync` keep that zone effectively ignored so the routing
//! contract and git's exclusion state never drift apart.

use std::cell::Cell;
use std::io::Write;
use std::process::exit;

use clap::{Parser, Subcommand};
use genesis::guide::{CliFormat, CliVerbosity, Output, OutputFormat, Verbosity};
mod cmds;

use whisper::{CLI_VERSION, entry};

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
pub(crate) enum Commands {
    /// Canonical repo key, branch slug, and worktree slot (pure determinism).
    Key,
    /// Resolve the exact destination path for a knowledge scope.
    Resolve {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
        /// Write to the machine-local store instead of the checkout's
        /// repo-local .whisper/ (repo/branch scopes only).
        #[arg(long)]
        global: bool,
        /// Write to the checkout's private zone .whisper/private/ —
        /// readable locally, structurally never pushed (repo/branch only).
        #[arg(long)]
        private: bool,
    },
    /// Append text to a scope's file (extend, don't duplicate).
    Append {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
        /// Text to append (positional alternative to --text).
        #[arg()]
        text: Option<String>,
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
        /// Write to the machine-local store instead of the checkout's
        /// repo-local .whisper/ (repo/branch scopes only) — escape hatch for
        /// read-only checkouts (CI, no-push contributors).
        #[arg(long)]
        global: bool,
        /// Write to the checkout's private zone .whisper/private/ —
        /// readable locally, structurally never pushed (repo/branch only).
        #[arg(long)]
        private: bool,
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
        /// Serve capped one-line heads (entries + freeform) instead of full
        /// entries — the fat-scope-safe default: one JSON line that stays
        /// under the ~50KB harness truncation limit.
        #[arg(long)]
        digest: bool,
        /// Include superseded entries.
        #[arg(long)]
        include_superseded: bool,
        /// Read-only recall: skip usage telemetry (sidecar untouched).
        #[arg(long)]
        no_usage: bool,
    },
    /// Two-phase distill contract: snapshot, agent rewrites, guarded commit.
    Distill {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
        /// Snapshot the current state and return the working paths.
        #[arg(long)]
        begin: bool,
        /// Install the distilled working file for this revision.
        #[arg(long)]
        commit: bool,
        /// Revision id (required with --commit).
        #[arg(long)]
        revision: Option<String>,
    },
    /// Transport-agnostic knowledge bundles (pack / unpack).
    Bundle {
        #[command(subcommand)]
        command: BundleCommand,
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
    /// Report a bug or feature request via GitHub issues.
    #[command(
        after_help = "Examples:\n  turu feedback bug --dry-run\n  echo \"steps to reproduce…\" | turu feedback bug\n  turu feedback bug --from-last-error"
    )]
    Feedback {
        /// Kind of feedback: bug, feature, question, or chore.
        #[arg(value_name = "KIND")]
        kind: String,
        /// Include the last error context from turu's error scratch.
        #[arg(long)]
        from_last_error: bool,
        /// Print the issue body and gh command without submitting.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum BundleCommand {
    /// Pack a scope into a deterministic bundle (keyed by repo key).
    Pack {
        /// Scope: global | repo | branch | worktree | group
        #[arg()]
        scope: String,
        /// Write the bundle to this file (deterministic bytes); default: envelope.
        #[arg(long)]
        out: Option<String>,
    },
    /// Merge a bundle into the local workspace (extend, never overwrite).
    Unpack {
        /// Bundle file to merge.
        #[arg(long)]
        file: Option<String>,
        /// Read the bundle from stdin instead of --file.
        #[arg(long)]
        stdin: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillCommand {
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
    match cmds::dispatch(&cli) {
        Ok(output) => emit_ok(&output),
        Err(err) => {
            // Adopt the genesis scratch contract: persist the last error on
            // non-zero exits so `turu feedback bug --from-last-error` has a
            // record to read (best-effort, never shadows the real error).
            let record = genesis::feedback::scratch::ErrorRecord {
                ts: scratch_timestamp(),
                argv: std::env::args().collect(),
                exit: 1,
                footer: err.suggestion.clone(),
                kind: "error".to_string(),
            };
            genesis::feedback::scratch::write_scratch_best_effort("turu", &record);
            let out = match &err.suggestion {
                Some(s) => Output::<String>::failure(err.message.clone()).with_next_step(s),
                None => Output::<String>::failure(err.message.clone()),
            };
            emit_err(&out);
            1
        }
    }
}

/// Second-precision UTC timestamp for error-scratch records (same shape as
/// the genesis Guide's ErrorSink writes and every other turu timestamp).
fn scratch_timestamp() -> String {
    crate::entry::parse_or_now(None).unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn emit_ok<T: serde::Serialize + std::fmt::Debug>(output: &Output<T>) -> i32 {
    emit(output, true)
}

fn emit_err(output: &Output<String>) -> i32 {
    emit(output, false)
}

/// Shared emit: success exits 0; failures exit 1 (write errors report and
/// exit 1 either way).
fn emit<T: serde::Serialize + std::fmt::Debug>(output: &Output<T>, success: bool) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let format = FORMAT.with(|f| f.get());
    let verbosity = VERBOSITY.with(|v| v.get());
    match output.emit(CLI_VERSION, format, verbosity, &mut stdout, &mut stderr) {
        Ok(()) if success => 0,
        Ok(()) => 1,
        Err(e) => {
            let _ = writeln!(stderr, "whisper: write error: {e}");
            1
        }
    }
}
