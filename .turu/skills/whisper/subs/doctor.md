<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/doctor
description: "Deep workspace diagnostics"
---

# whisper/doctor

Deep workspace diagnostics.

## Protocol

1. `turu doctor --json` — layout, group health, legacy keys, managed block, skill pack staleness
2. Each failing check carries a `fix` hint; apply it
3. Privacy trio: `turu.private-ignore` (effective ignore via git check-ignore — text presence is not the truth), `turu.private-tracked` (FAIL when private knowledge is in history — git history is forever — prevention only), `turu.private-leaks` (advisory lint for machine paths, hostnames, token-shaped strings in public files)
