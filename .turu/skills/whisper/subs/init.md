<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/init
description: "Create the workspace layout for this checkout"
---

# whisper/init

Create the workspace layout for this checkout.

## Protocol

1. `turu init --json` — creates rules.md, repo slot, branch slot, worktree slot, and the machine-local store slots
2. Never overwrites existing files; ensures `.whisper/private/` is effectively ignored (appends the rule to the root .gitignore only when git check-ignore doesn't already report it)
3. `turu sync` afterwards to (re)install the AGENTS.md managed block
