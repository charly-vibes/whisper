<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/key
description: "Derive canonical repo key, branch slug, and worktree slot"
---

# whisper/key

Derive canonical repo key, branch slug, and worktree slot.

## Protocol

1. `turu key --json` — deterministic; same repo → same key on every machine
2. Never derive these paths by hand while the binary is present
