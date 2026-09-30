# CLI reference

All commands emit a genesis JSON envelope. Scopes: `global`, `repo`, `branch`,
`worktree`, `group` (recall also accepts `all`).

## `turu key`

Canonical repo key, branch slug, and worktree slot for the current checkout
(pure determinism core — a pure function of the remote URL).

## `turu resolve <scope>`

The exact destination path for a knowledge scope.

- `--global` — write to the machine-local store instead of the checkout's repo-local `.whisper/` (repo/branch scopes only)
- `--private` — write to the checkout's private zone `.whisper/private/` — readable locally, structurally never pushed (repo/branch only)

## `turu append <scope> --text "..."`

Append text to a scope's file (extend, don't duplicate). Entries are
structured and idempotent: sha2 ids over scope + second-precision UTC + text;
freeform content preserved verbatim.

- `--text` — text to append (repeatable)
- `--stdin` — read the text from stdin instead of `--text`
- `--topic <k>` — optional topic key (bare word, no whitespace/parens/#)
- `--supersedes <id>` — mark the entry with this id as superseded by the new entry
- `--global` / `--private` — as `resolve`

## `turu recall <scope>`

Serve a ranked, budget-bounded slice of a scope's entries. Whole entries only
(never truncated); reports `served_bytes`, `budget_unused`, `entries_skipped`.

- scope may be `all` — composes scopes in precedence order
- `--topic <k>` — only entries with this topic key
- `--budget <bytes>` — byte budget
- `--include-superseded` — include superseded entries
- `--read-only` — skip usage telemetry (sidecar untouched)

## `turu distill <scope>`

Two-phase distill contract: snapshot, agent rewrites, guarded commit.

- `--begin` — snapshot the current state and return the working paths
- `--commit --revision <id>` — install the distilled working file for this revision
- Guards the commit against concurrent drift between snapshot and commit

## `turu bundle pack|unpack`

Deterministic, transport-agnostic knowledge bundles keyed by repo key.
Unpack merges extend-without-duplicate and never overwrites.

## `turu init`

Create the workspace layout for this checkout (never overwrites).

## `turu status`

One envelope with every relevant path and existence flag.

## `turu check`

Detect legacy key variants, undefined groups, and missing files.

## `turu consolidate`

Migrate legacy repo-key directories into the canonical key.

## `turu doctor`

Deep workspace diagnostics: layout integrity, group health, legacy keys,
managed block, skill pack staleness, and the private-zone contract
(`private-ignore`, `private-tracked`, `private-leaks`).

## `turu sync`

Inject/refresh the turu managed block (`<!-- TURU:START -->`) in the
agent-facing file (default: `AGENTS.md` at the repo root), so agents read
paths from a file instead of re-deriving them.

## `turu feedback <kind>`

File a well-contexted GitHub issue (`bug`, `feature`, `question`, `chore`):
redacted context bundle, optional `--from-last-error` scratch recall,
`--dry-run` preview (via `genesis::feedback`).

## `turu skill install`

Ship the whisper skill from this repo: router + sub-skills (including manual
fallbacks), content-hashed so `turu doctor` flags staleness. Targets
`~/.claude/skills` or any dir; incitaciones keeps a thin pointer to it.
