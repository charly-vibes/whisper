# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-09-17

### Added

- `turu.shadowed-global` doctor check (`whisper-122`) — when a repo-private `workspace_root` override is active, surfaces that it shadows the global scope: a hidden global `rules.md` warns, diverged copies warn with both paths, in-sync copies pass informationally
- `turu check` warns when the repo-private root hides or diverges from the global `rules.md` (previously only `turu doctor` reported it)

### Fixed

- A repo-private `workspace_root` with a **relative** path now anchors to the directory containing the repo config instead of the process cwd (GH #1) — `turu` invoked from a subdirectory no longer materializes a stray `<subdir>/.whisper/` workspace tree; absolute and `~` paths are unchanged

## [0.4.0] - 2026-09-15

### Added

- Structured, idempotent knowledge entries (`add-entry-model`): every appended fact becomes a ledger line carrying a full sha2 id (scope + second-precision UTC + text), a timestamp, an optional `--topic` key, and mechanical `--supersedes` marking; duplicate appends are no-ops reported as `duplicate`; freeform content is preserved verbatim; `TURU_NOW` env override gives reproducible timestamps
- `turu recall <scope> [--topic] [--budget] [--include-superseded]` (`add-recall-serving`) — ranked, whole-entry-atomic slice serving; `all` composes scopes in precedence order; the envelope reports the context-horizon boundary (`served_bytes`, `budget_unused`, `entries_skipped`, `freeform_skipped`) so the agent can decide when to load memory at all
- `turu distill <scope> --begin|--commit --revision <id>` (`add-distill-contract`) — two-phase contract: turu snapshots the scope into immutable revisions and guards the commit against concurrent drift; the calling agent performs the semantic rewrite into the working file; replay is a reported no-op; revisions are never pruned
- `turu bundle pack|unpack` (`add-shared-bundle`) — deterministic, transport-agnostic knowledge bundles keyed by the canonical repo key; unpack merges extend-without-duplicate and never overwrites (strategic transport fork documented as open in `openspec/changes/add-shared-bundle/design.md`)
- doctor checks: `turu.entry-format` (unmanaged freeform lines), `turu.distill-pending` (revisions begun but not committed)
- `openspec/` baseline specs for existing behavior (workspace-routing, knowledge-append, config-precedence, managed-block, workspace-maintenance) + lifecycle change proposals

### Changed

- `turu append` writes structured entry lines instead of verbatim text; appends use a single O_APPEND write so concurrent agents cannot lose each other's entries (full rewrite only for `--supersedes` marking); multi-line input is normalized (per-line leading whitespace stripped, id computed over the normalized text)
- envelope `appended_bytes` reports the rendered line length actually written, not the raw input length
- entry parsing requires canonical RFC-3339 seconds timestamps — malformed lines degrade to unmanaged freeform instead of corrupting recency ordering

## [0.3.0] - 2026-02-20

### Added

- `turu skill install [<dir>]` — first-party whisper skill pack shipped from this repo (dont's `skill_pack` pattern): deterministic generation, content hashes, 11 files (router + 10 sub-skills including manual fallbacks for link/decommission/consolidate)
- `turu.managed-skills` doctor check — flags stale packs via content hashes (missing pack passes as optional)
- Incitaciones whisper skill converted to a thin pointer that delegates to the CLI

## [0.2.0] - 2026-02-20

### Added

- `turu doctor` — deep workspace diagnostics via genesis doctor conventions: rules file, repo/branch slots, legacy key variants, group health, managed block presence (each check carries a fix hint)
- `turu sync` — inject/refresh the `<!-- TURU:START -->` managed block in AGENTS.md with the deterministic routing map, so agents read paths from a file instead of re-deriving them

### Changed

- **Main command renamed to `turu`** (the song's turututu riff); `turututu` and `whisper` ship as alias bins

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
