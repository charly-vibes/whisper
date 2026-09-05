<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/consolidate-manual
description: "Migrate legacy repo-key directories to the canonical key (manual fallback)"
---

# whisper/consolidate-manual

Migrate legacy repo-key directories to the canonical key (manual fallback).

## Protocol

1. `turu doctor --json` lists variants under `turu.legacy-keys`
2. Move directory contents into the canonical key dir, merge env.md by extending, never duplicating
3. Re-run `turu doctor` — the check must pass
