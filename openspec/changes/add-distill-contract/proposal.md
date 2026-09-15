# Add Distill Contract — turu-owned mechanics for re-synthesis

## Why

The memory-systems analysis (CORR-002, EDGE-001) shows whisper has a write path but no manage/distill stage, so staleness accumulates forever (§1 Khemani: both ChatGPT and Claude converged on **periodic async re-synthesis**). But a distill is *destructive*: an LLM rewrite can drop still-relevant facts, turning staleness horror stories into silent data-loss stories. The fix must keep determinism:

- **turu owns the mechanics**: snapshot, revision, conflict detection, install, audit trail.
- **The skill/LLM owns the judgment**: what to merge, dedupe, drop, rewrite.
- Nothing is deleted: pre-distill snapshots are immutable revisions.

This mirrors how the AGENTS.md managed block already works — turu owns format and placement, the content is authored elsewhere.

## What Changes

- New two-phase command `turu distill <scope>`:
  - `--begin` snapshots the scope's current state (hash + copy), returns the working paths and the snapshot hash in the envelope.
  - The calling agent performs the semantic rewrite into the working path.
  - `--commit --revision <id>` installs the distilled result **only if** the live files are unchanged since `--begin` (hash check); on drift it refuses with a conflict suggestion and nothing is overwritten.
- Revisions are retained: two consecutive distills produce two distinct snapshots; turu never deletes a revision.
- Purge of old revisions is out of scope (a later change may add age-based pruning; never by default).

## Capability

New capability: `distillation`.

## Impact

- Affected specs: none (new capability).
- Affected code: `src/main.rs` (new `Commands::Distill`), new revision/audit module (reuses sha2).
- Depends on: `add-entry-model` (the distill input format is the entry ledger; freeform lines pass through unchanged).
- Breaking? No — pure addition.
