# design.md — add-entry-model

## Entry id derivation

Inputs to the sha2 hash, in order: scope key (e.g. repo key or `global`), the append timestamp, and the exact appended text. Timestamps are **second-precision UTC** (RFC-3339, seconds): two agents appending the same text within the same wall-clock second receive the same id — that *is* the idempotency guarantee; cross-second re-statements are new entries. Consequence: identical text appended at different times gets different ids — idempotency protects against *concurrent double-append of the same event*, not against re-stating a fact later (stating it later is a new entry; conflict resolution is distill's job, not the id's). Rejected alternative: hash of scope+content only — would make re-stating undetectable and would break legitimate "same text, different context" cases.

## Entry format

Structured markdown line, machine-parseable, human-readable:

```
- <RFC-3339 seconds UTC> [id:<full-sha2-hex>] (#topic)? text… (supersedes <id>)?

Ids are the **full sha2 hex** — no truncation — so envelope references, supersede targets, and bundle ids are all the same unambiguous string. (Truncated display prefixes would need a collision rule; not worth it.)
```

Rejected alternatives: front-matter blocks (too heavy per entry); JSONL sidecar (two sources of truth, violates one-file-per-scope routing). Trailing format decision lives in the red/green tests — spec pins behavior, not exact syntax.

## Freeform legacy lines

Lines without the id marker are treated as unmanaged text: doctor warns, recall/distill pass them through unchanged. No migration is forced.

## Concurrency

Idempotent append covers the realistic race (two agents append the same fact in the same second). Appends use a **fast path** — a single O_APPEND write of the rendered entry line — so concurrent writers cannot lose each other's entries. Only the rare `--supersedes` marking rewrites the whole file (it must mutate a line in place); during that window a concurrent append can be lost. Accepted residual risk: marking is an explicit, rare operation.

## Continuation lines

Entry text lines fold only when the file line starts with **exactly** two spaces (the renderer's convention). Deeper indents — 4-space markdown code blocks — stay unmanaged freeform, so indented content after an entry is never absorbed and reformatted. The residual edge (entry text whose own lines begin with whitespace) is resolved at ingest: `append_entry` strips per-line leading whitespace and computes the id over the normalized text, so nothing that enters the ledger can break the fold rule.
