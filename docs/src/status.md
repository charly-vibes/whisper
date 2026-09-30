# Status

`whisper` **v0.8.0** — Status: **beta**. Core (key/resolve/append/recall/distill)
works and is used in anger by other ecosystem tools; surface may still shift
before 1.0.

## Implemented

| Command | Status | Notes |
|---|---|---|
| `turu key` | stable | pure function of remote URL; determinism core |
| `turu resolve` | stable | all five scopes + `--global`/`--private` |
| `turu append` | stable | idempotent sha2 ids, topics, supersede chain |
| `turu recall` | stable | budget-bounded slices, `all` precedence composition, horizon boundary reported |
| `turu distill` | stable | two-phase begin/commit with drift guard |
| `turu bundle` | stable | pack/unpack, extend-without-duplicate merge |
| `turu init` | stable | never overwrites |
| `turu status` | stable | single envelope, existence flags |
| `turu check` | stable | legacy keys, undefined groups, missing files |
| `turu consolidate` | beta | migration of legacy repo-key dirs |
| `turu doctor` | stable | layout, groups, managed block, skill staleness, privacy contract |
| `turu sync` | stable | TURU managed block in AGENTS.md |
| `turu feedback` | beta | genesis::feedback, kinds bug/feature/question/chore |
| `turu skill install` | beta | router + sub-skills, content-hashed staleness check |

## In progress

- Consolidate rollout across repos with legacy key directories (see the
  whisper workspace hygiene item in incitaciones' inbox).

## Mapped to specs

- Envelope/config/feedback conventions: [genesis-vibes](https://github.com/charly-vibes/genesis)
- Skill contract: incitaciones `whisper` skill (thin pointer to `turu skill install` output)

## Dogfooding

- **incitaciones** — the distilled `whisper` skill is a thin pointer to this
  repo's skill pack (`turu skill install`)
- **session/renew skill** (org agent config) — calls `turu key`, `turu recall
  repo/branch` for session history before journal greps
- whisper's own CI runs `just ci` (fmt, clippy, tests, locked build); pinned
  ecosystem installs tracked in `versions.ddl.toml`
