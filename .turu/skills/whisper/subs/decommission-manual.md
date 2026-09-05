<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/decommission-manual
description: "Decommission a branch/worktree slot (manual fallback)"
---

# whisper/decommission-manual

Decommission a branch/worktree slot (manual fallback).

## Protocol

1. Confirm the branch is merged/decommissioned in beads if available
2. Remove the slot directory under the branch/worktree root (canonical key from `turu key`)
3. Keep env.md knowledge by promoting it: repo-wide facts → `turu append repo`, global → `turu append global`
