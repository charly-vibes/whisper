# Add Entry Model — structured, idempotent knowledge entries

## Why

Today `turu append` extends freeform text; scope files (`env.md`, `notes.md`) are unaddressable blobs. Three consequences observed in the memory-systems analysis (Rule-of-5 review, findings CORR-001, EDGE-002):

1. **No recall substrate** — planned slice-based recall (`add-recall-serving`) needs per-entry ids, topics, and ordering; impossible over freeform text.
2. **No distill substrate** — planned distill contract (`add-distill-contract`) needs a format it can rewrite against and verify.
3. **No concurrent-write safety** — multiple agents appending to one branch file can interleave and duplicate facts (the exact "bad memory is expensive" failure the report warns about).

This change is the **substrate**: the other two lifecycle changes are consumers of it.

## What Changes

- Entries appended via `turu append` gain a deterministic id (hash of scope + timestamp + content, using the existing `sha2` dep) and carry-through metadata: timestamp, topic key, optional `supersedes` reference.
- Append becomes **idempotent**: appending an entry whose id already exists is a no-op reported as `duplicate` in the envelope.
- Files stay human-readable markdown — entries are structured markdown lines/blocks, not a binary format.
- `supersedes` marking is mechanical (turu records the reference, never interprets content).

## Capability

New capability: `entry-model`.

## Impact

- Affected specs: none (new capability).
- Affected code: `src/workspace.rs` (`Target::append`), `src/main.rs` (`Commands::Append`).
- Enables: `add-recall-serving`, `add-distill-contract` (both list this as a dependency).
- Breaking? No — existing appends still work; freeform lines remain valid (they simply have no id until re-written by a later distill).
