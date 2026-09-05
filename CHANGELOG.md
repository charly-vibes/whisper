# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-20

### Added

- `whisper key` — canonical repo key, branch slug, and worktree slot derived deterministically from git facts
- `whisper resolve <scope>` — exact destination path for a knowledge scope (`global | repo | branch | worktree | group`)
- `whisper append <scope>` — verbatim extend-don't-duplicate writes with parent creation
- `whisper init` — workspace layout creation, never overwrites
- `whisper status` — full path map and existence flags in one envelope
- `whisper check` — legacy key-variant detection and missing-file warnings
- Layered config: global (`~/.config/whisper/config.toml`) > group (`[groups.*]`) > repo (`.whisper/config.toml`, gitignored)
- Genesis envelope output: `--json` / `--human` with progressive verbosity
