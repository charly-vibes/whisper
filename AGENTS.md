<!-- WAI:REFLECT:REF:START -->
## Accumulated Project Patterns

Project-specific conventions, gotchas, and architecture notes live in
`.wai/resources/reflections/`. Run `wai search "<topic>"` to retrieve relevant
context before starting research or creating tickets.

> **Before research or ticket creation**: always run `wai search "<topic>"` to
> check for known patterns. Do not rediscover what is already documented.


<!-- provenance: generator=wai version=0.12.3 source=WAI:REFLECT:REF sha=73879972 -->
<!-- WAI:REFLECT:REF:END -->

<!-- ah:managed:start -->
## espectacular

Run `ah check` to verify spec-test correspondence before committing.

- `ah check` — validate all deployed specs
- `ah check --changes <name>` — validate with a change overlay
- `ah init` — set up or refresh espectacular project files
- `ah doctor` — diagnose setup issues
- `ah explain <topic>` — playbook guidance for finding kinds and suggested actions
- `ah doctor --enable <adapter>` — write adapter config into .espectacular/config.toml
- `ah signals` — emit dont drift signals
<!-- ah:managed:end -->

<!-- BEADS:START -->
<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY.
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session
<!-- END BEADS INTEGRATION -->
<!-- BEADS:END -->

<!-- DONT:START -->
<!-- dont: managed block -->
See .dont/ for grounded-claim workflow.
<!-- DONT:END -->

<!-- WAI:START --># Workflow Tools

This project uses **wai** to track the *why* behind decisions — research,
reasoning, and design choices that shaped the code. Run `wai status` first
to orient yourself.

Detected workflow tools:
- **wai** — research, reasoning, and design decisions
- **beads** — issue tracking (tasks, bugs, dependencies). CLI command: **`bd`** (not `beads`)
- **openspec** — specifications and change proposals (see `openspec/AGENTS.md`)

> **CRITICAL**: Apply TDD and Tidy First throughout — not just when writing code:
> - **Planning/task creation**: each ticket should map to a red→green→refactor cycle; refactoring tasks must be separate tickets from feature tasks.
> - **Design**: define the test shape (inputs/outputs) before designing the implementation.
> - **Implementation**: write the failing test first, then make it pass, then tidy in a separate commit.

> **When beginning research or creating a ticket**: run `wai search "<topic>"` to check for existing patterns before writing new content.

## Quick Start

1. `wai sync` — ensure agent tools are projected
2. `wai status` — see active projects, phase, and suggestions
3. `bd ready` — find available work items

When context reaches ~40%: stop and tell the user — responses degrade past
this point. Recommend `wai close` then `/clear` to resume cleanly.
Do NOT skip `wai close` — it enables resume detection.

## Autonomous Work Policy

Proceed without routine confirmation when the next step is clear.
Do not ask to continue, fix, or commit — just do it.

**Stop and ask** only when:
- Conflicting requirements or ambiguous intent
- Destructive actions (data loss, force-push, drop table)
- Credentials, secrets, or external services not yet authorized
- Unresolved test failures after two attempts
- Push, deploy, or release — always get explicit authorization
- Context approaching 40% — recommend `wai close` then `/clear`

## Detailed Instructions

Full workflow reference — session lifecycle, capturing work, command cheat
sheets, cross-tool sync, and PARA structure — lives in **`.wai/AGENTS.md`**.
Read it at the start of your first session or when you need detailed guidance.

Keep this managed block so `wai init` can refresh the instructions.


<!-- provenance: generator=wai version=0.12.3 source=WAI sha=3e02f7c4 -->
<!-- WAI:END -->

<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Whisper — Deterministic Knowledge Workspace

Rust CLI on `genesis-vibes` that makes the incitaciones `whisper` skill deterministic: canonical repo keys, branch slugs, worktree slots, and scope-based knowledge routing as pure binary behavior instead of LLM-improvised shell pipelines.

> **CRITICAL**: Apply TDD and Tidy First throughout — each ticket maps to a red→green→refactor cycle; refactoring tasks are separate tickets from feature tasks.

## Conventions

- **genesis-vibes is the shared foundation** — use `envelope`, `guide` (`CliVerbosity`, `CliFormat`, `Output`), `config`, and `managed_block` conventions. Do not hand-roll output envelopes or config parsing.
- **The command is `turu`** (song: turututu riff); `turututu` and `whisper` are alias bins. Keep all three working.
- All output goes through the genesis JSON envelope — `--json` is the machine contract, `--human` the default.
- Canonical repo key derivation is a **pure function** of the remote URL. Never machine-specific, never invented. See `src/workspace.rs`.
- The `<!-- TURU:START -->` managed block in `AGENTS.md` is the agent-facing contract — regenerate only via `turu sync`.

## Quick Reference

```bash
just build     # cargo build
just test      # cargo test
just lint      # clippy -D warnings
just run key   # try the core determinism command
just run sync  # refresh the AGENTS.md managed block
```

## Config Model

Precedence: repo (`.whisper/config.toml`, gitignored) > group (global `[groups.*]`) > global (`~/.config/whisper/config.toml`). Groups route a set of repos into one shared knowledge directory.

<!-- TURU:START -->
# Whisper knowledge workspace (managed by turu — regenerate with `turu sync`)

- Workspace root: `/var/home/sasha/.whisper`
- Repo key: `cv/charly-vibes/whisper` · Branch slug: `main` · Worktree slot: `.git`
- Deterministic routing (resolve, never guess):
- global → `/var/home/sasha/.whisper/rules.md`
- repo → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/env.md`
- branch → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/branches/main/notes.md`
- worktree → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/worktrees/.git/env.md`
  - append/resolve default: repo → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/env.md`, branch → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/branches/main/notes.md`
  - `--global` escape hatch: repo → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/env.md`, branch → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md`
  - `--private` zone (never pushed): repo → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/private/env.md`, branch → `/var/home/sasha/para/areas/dev/gh/charly/whisper/.whisper/private/branches/main/notes.md`
- Commands: `turu resolve <scope>` · `turu append <scope> [--global] --text ... [--topic k] [--supersedes id]` · `turu recall <scope> [--topic k] [--budget bytes]` · `turu distill <scope> --begin|--commit` · `turu bundle pack|unpack` · `turu status` · `turu doctor` · `turu feedback <kind>`

<!-- TURU:END -->

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
<!-- DDL:START -->

## dulce-de-leche (ddl v0.8.0)

Orchestrator for the charly-vibes tool ecosystem. Run `ddl init` to
install and configure all tools. Run `ddl status` for a health overview.

Commands:
- init     Bootstrap the toolset
- install  Install a single tool
- status   Show health overview
- doctor   Run diagnostics
- version  Show versions
- upgrade  Update tools
- migrate  Migrate configs
- scope    Show active .ddl/

<!-- DDL:END -->
