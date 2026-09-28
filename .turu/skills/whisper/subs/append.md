<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->

---
name: whisper/append
description: "Extend-don't-duplicate knowledge into the right file"
---

# whisper/append

Extend-don't-duplicate knowledge into the right file.

## Protocol

1. Search first — check the target file for an existing note on the topic
2. `turu append <scope> --text "..."` — appends verbatim, creates parents. In a checkout, repo/branch scopes default to the checkout's own `.whisper/` (transport decision A: the repo's git is the transport)
3. `--global` writes the machine-local store instead — the fallback for read-only checkouts (CI, no-push contributors); `--private` writes `.whisper/private/`, gitignored, never pushed
4. Extend or correct existing notes instead of adding duplicates; no secrets, ever — git history is forever — prevention only
