# design.md — add-distill-contract

## Why two phases

A pure binary cannot summarize text — semantic distillation is the skill's judgment. But letting the LLM write scope files directly destroys the audit trail and risks dropping live facts. The two-phase contract splits ownership cleanly: turu guarantees *what was there*, *what changed*, and *nothing silently lost*; the agent owns *what the new text says*.

## Revision model

`--begin` copies the scope's files to `revisions/<utc-timestamp>-<hash-prefix>/` inside the scope's directory tree, recording: revision id, per-file sha2, and the working paths returned in the envelope. - Revisions are immutable: `--commit` never touches them, and a second distill creates a new revision dir. Revision location is uniform: `revisions/` next to the scope's target file (so `global` → `<workspace-root>/revisions/`, repo → `repos/<key>/revisions/`, etc.). Rejected alternative: git commits inside `~/.whisper` (workspace is not required to be a git repo; keep zero-git assumption).

## Conflict detection

`--commit` re-hashes the live files; any drift since `--begin` (e.g. a concurrent `turu append` during the rewrite) aborts the commit with a `next_step` suggesting re-running `--begin`. Overwrite-on-drift is the failure mode this contract exists to prevent (EDGE-001). The `--revision` argument is validated against a strict grammar (`<compact-timestamp>-<8 hex>`), which also closes path traversal via a crafted revision id.

## Install semantics

On commit, the distilled file replaces the live file atomically (write-temp + rename) and the envelope reports: revision id, files replaced, snapshot path. Idempotent replay of the same commit with no further drift is a no-op reported as such.

## What turu does NOT do

- No content interpretation: turu cannot judge whether the distilled text dropped a relevant fact — that risk is mitigated by immutable snapshots, not by turu semantics.
- No scheduling: when distill runs ("dreaming" cadence) is the skill's decision, not a daemon.
