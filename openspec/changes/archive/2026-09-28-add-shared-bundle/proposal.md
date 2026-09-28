# Add Shared Bundle — deterministic knowledge export/import between workspaces

## Why

The memory-systems analysis (EXCL-002) flags the existential question: §2 Malcolm — "file-based memory works for one agent, breaks past one," and the motivating war story is *shared* context ("git records the code and not human intent"). Whisper today is per-machine, per-user (`~/.whisper`); branch-scoped notes die on one laptop.

The **strategic fork** (whether whisper becomes team-shared infrastructure, and via which transport) must be decided separately — see design.md. But every option under consideration needs the same deterministic primitive first: a bundle format keyed by the canonical repo key, mergeable with the extend-without-duplicate rules that `turu consolidate` already established.

## What Changes

- New subcommand `turu bundle`:
  - `bundle pack <scope>` produces a single deterministic archive (JSON envelope + files) keyed by repo key, containing the scope's files and entry ids.
  - `bundle unpack <archive>` merges into the local workspace using consolidate's merge rules (rename-if-absent, extend-without-duplicate); duplicate entry ids are skipped and reported.
- Transport is **out of scope**: how bundles travel (committed into the repo, git refs, separate remote) is the open strategic decision. The bundle is transport-agnostic.

## Capability

New capability: `shared-store` (bundle primitive only).

## Impact

- Affected specs: none (new capability).
- Affected code: `src/main.rs` (new `Commands::Bundle`), reuses `workspace::consolidate` merge logic.
- Depends on: `add-entry-model` (entry ids make duplicate-skip deterministic rather than line-based).
- Breaking? No.
