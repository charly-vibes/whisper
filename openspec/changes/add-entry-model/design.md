# design.md — add-entry-model

## Entry id derivation

Inputs to the sha2 hash, in order: scope key (e.g. repo key or `global`), RFC-3339 timestamp of the append, and the exact appended text. Consequence: identical text appended at different times gets different ids — idempotency protects against *concurrent double-append of the same event*, not against re-stating a fact later (stating it later is a new entry; conflict resolution is distill's job, not the id's). Rejected alternative: hash of scope+content only — would make re-stating undetectable and would break legitimate "same text, different context" cases.

## Entry format

Structured markdown line, machine-parseable, human-readable:

```
- <RFC-3339> [id:<first-12-hash-chars>] (#topic)? text… (supersedes <id>)?
```

Rejected alternatives: front-matter blocks (too heavy per entry); JSONL sidecar (two sources of truth, violates one-file-per-scope routing). Trailing format decision lives in the red/green tests — spec pins behavior, not exact syntax.

## Freeform legacy lines

Lines without the id marker are treated as unmanaged text: doctor warns, recall/distill pass them through unchanged. No migration is forced.

## Concurrency

Idempotent append covers the realistic race (two agents append the same fact in the same second). Full file locking is out of scope — noted as a follow-up if multi-writer usage materializes.
