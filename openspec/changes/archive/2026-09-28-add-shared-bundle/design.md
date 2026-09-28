# design.md — add-shared-bundle

## The strategic fork (decide separately from this change)

| Option | Mechanism | Pros | Cons |
|---|---|---|---|
| A. In-repo dir | `.whisper/` committed to the repo | zero transport, branch-scoped notes visible to teammates via git | couples knowledge to repo history; per-branch noise in PRs |
| B. Git refs protocol | bundles under `refs/` on the existing remote (beads-style) | no working-tree noise; per-repo scoping preserved | needs push/pull commands; more moving parts |
| C. Shared remote store | central ~/.whisper-like service | true team memory incl. cross-repo | violates ownership maxim (§2 Chase); infrastructure to run |

This change deliberately ships only the **bundle primitive** all three options consume. The fork decision (A/B/C, or none) is a separate change proposal once real multi-machine/team usage materializes — do not build transport speculatively.

## Bundle format

A single JSON envelope (genesis conventions) wrapping: repo key, scope, entries with ids, and raw file contents. Deterministic ordering (entries by id) so packing the same state twice yields identical bytes — bundles are diff-able and hash-addressable.

## Merge semantics

Reuse `consolidate`'s rules: absent target → place file; existing target → extend with non-duplicate lines; entry id already present → skip and report in envelope (`duplicates_skipped`). Conflicting *content* under the same path is resolved by extend, never overwrite — mirrors the legacy-migration precedent and keeps unpack non-destructive (a weaker cousin of the distill revision guarantee).
