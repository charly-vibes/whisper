# Add Recall Serving — scoped slice retrieval with budget boundaries

## Why

The memory-systems analysis (Rule-of-5 review, DRAFT-003, EXCL-001) shows whisper is a **Record-only** tool: routing answers *where a note lives*, not *what gets loaded into context*. The report's hardest evidence says this is the gap that matters:

- Druga (§3): a ranked decisions-ledger recall policy beat vector RAG **and even oracle injection** — recall policy is the variable, not storage.
- Chin (§4): whole-file markdown loading wastes tokens ("~100k per round in the hopes that something will be useful").
- Khemani/report cross-cutting #5: memory adds nothing when the task fits in context — the tool should make the boundary *visible*, not hide it.

## What Changes

- New command `turu recall <scope>` serving **slices** of a scope's entries, filtered by `--topic`, bounded by `--budget` (bytes), ranked by recency, excluding superseded entries by default.
- The envelope reports the context-horizon boundary explicitly: bytes served, budget unused, entries skipped — so the calling agent can decide to skip loading entirely when content fits.
- When recall spans multiple scopes, deterministic precedence (global < group < repo < branch < worktree) is reported per entry in the envelope — the agent never guesses which note wins.
- **Policy stays agent-owned**: turu serves mechanically (filter, rank, budget); *when* to call recall remains the agent's decision. This preserves the deterministic-half boundary.

## Capability

New capability: `recall`.

## Impact

- Affected specs: none (new capability).
- Affected code: `src/main.rs` (new `Commands::Recall`), new slice-serving module.
- Depends on: `add-entry-model` (entry ids, topics, supersede marks are the substrate).
- Breaking? No — pure addition.
