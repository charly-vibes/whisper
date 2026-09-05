# Whisper — Deterministic Knowledge Workspace

Rust CLI on `genesis-vibes` that makes the incitaciones `whisper` skill deterministic: canonical repo keys, branch slugs, worktree slots, and scope-based knowledge routing as pure binary behavior instead of LLM-improvised shell pipelines.

> **CRITICAL**: Apply TDD and Tidy First throughout — each ticket maps to a red→green→refactor cycle; refactoring tasks are separate tickets from feature tasks.

## Conventions

- **genesis-vibes is the shared foundation** — use `envelope`, `guide` (`CliVerbosity`, `CliFormat`, `Output`), and `config` conventions. Do not hand-roll output envelopes or config parsing.
- Binary is `whisper`; crate is `whisper-vibes` (the `whisper` name is taken on crates.io).
- All output goes through the genesis JSON envelope — `--json` is the machine contract, `--human` the default.
- Canonical repo key derivation is a **pure function** of the remote URL. Never machine-specific, never invented. See `src/workspace.rs`.

## Quick Reference

```bash
just build     # cargo build
just test      # cargo test
just lint      # clippy -D warnings
just run key   # try the core determinism command
```

## Config Model

Precedence: repo (`.whisper/config.toml`, gitignored) > group (global `[groups.*]`) > global (`~/.config/whisper/config.toml`). Groups route a set of repos into one shared knowledge directory.
