<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/resolve
description: "Get the exact write destination for a knowledge scope"
---

# whisper/resolve

Get the exact write destination for a knowledge scope.

## Protocol

1. `turu resolve global|repo|branch|worktree|group --json`
2. If a `<!-- TURU:START -->` block exists in AGENTS.md, read the routing map from there
3. Group scope routes to the shared group workspace when one is active
