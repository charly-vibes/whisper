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
- repo → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/env.md`
- branch → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/branches/main/notes.md`
- worktree → `/var/home/sasha/.whisper/repos/cv/charly-vibes/whisper/worktrees/.git/env.md`
- Commands: `turu resolve <scope>` · `turu append <scope> --text ...` · `turu status` · `turu doctor`

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
