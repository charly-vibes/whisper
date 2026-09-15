# Project Context

## Purpose

`whisper-vibes` (bins: `turu`, `turututu`, `whisper`) is the deterministic knowledge-workspace CLI for AI agents: canonical repo keys, branch slugs, worktree slots, and scope-based knowledge routing as pure binary behavior — the mechanical half of the `whisper` skill. The LLM/skill owns judgment; turu owns mechanics.

## Tech Stack

- Rust 2024, clap 4 (derive), serde/serde_json, toml, sha2, genesis-vibes 0.6 (envelope, guide, config, managed_block conventions)

## Project Conventions

### Code Style

- All output goes through the genesis JSON envelope: `--json` is the machine contract, `--human` the default. Never hand-roll envelopes or config parsing.
- The command is `turu` (song: turututu riff); `turututu` and `whisper` are alias bins — keep all three working.

### Architecture Patterns

- Canonical repo key derivation is a **pure function** of the remote URL (`src/workspace.rs`). Never machine-specific, never invented.
- Config precedence: repo (`.whisper/config.toml`, gitignored) > group (global `[groups.*]`) > global (`~/.config/whisper/config.toml`).
- Knowledge routing scopes: global / group / repo / branch / worktree — resolved, never guessed.
- The `<!-- TURU:START -->` managed block in `AGENTS.md` is the agent-facing contract; regenerated only via `turu sync`.
- `turu consolidate` migrates legacy repo-key dirs by rename/extend-without-duplicate — the precedent for any merge behavior.

### Testing Strategy

- TDD + Tidy First: each ticket maps to a red→green→refactor cycle; refactoring tasks are separate tickets from feature tasks.
- `just build` / `just test` / `just lint` (clippy `-D warnings`).
- Integration tests via `assert_cmd` + `predicates` + `tempfile` in `tests/`.
