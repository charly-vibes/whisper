<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper
description: "Deterministic knowledge workspace: init, check, status, link, decommission, knowledge routing. Delegate to the turu CLI; manual fallbacks when absent. Trigger on '/w', '/whisper', 'init workspace', 'check workspace', 'workspace status', 'link plan', 'decommission'."
tools: Read, Write, Edit, Bash
---

# Whisper — Deterministic Operational Knowledge

Manage the whisper knowledge workspace. The `turu` CLI owns all mechanical
steps: canonical repo keys, branch slugs, worktree slots, scope routing.
Never hand-roll those paths while the binary is present.

## Decision procedure

1. Is `turu` available? (`turu key --json` succeeds)
2. If **yes** — everything mechanical is a turu command:

| Intent | Command |
|---|---|
| Derive repo key / branch slug / worktree slot | `turu key --json` |
| Find the write destination for a scope | `turu resolve <scope> --json` |
| Record knowledge (extend, don't duplicate) | `turu append <scope> --text "..."` |
| Create the workspace layout | `turu init --json` |
| Inspect paths and existence | `turu status --json` |
| Validate / detect legacy keys | `turu check --json` · `turu doctor --json` |
| Refresh the AGENTS.md routing block | `turu sync --json` |

   Scopes: `global` (rules.md), `repo` (env.md), `branch` (notes.md),
   `worktree` (env.md), `group` (shared workspace). If a
   `<!-- TURU:START -->` block exists in AGENTS.md, read the routing map
   from there instead of running resolve.

3. If **no** — use the manual fallbacks in `subs/*-manual.md` and the
   incitaciones references; suggest installing via `cargo install whisper-vibes`.

## Invariants (with or without the binary)

- **No secrets anywhere.** Never write tokens, keys, passwords, or PII.
- **Extend, don't duplicate.** Append to or correct existing notes.
- **One repo, one key.** Canonical key is a pure function of the remote URL.
- **Search first** before creating a new entry.
