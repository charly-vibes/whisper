# distillation — Spec Delta (proposed by add-distill-contract)

## ADDED Requirements

### Requirement: Two-phase distill contract

`turu distill <scope>` SHALL operate in two deterministic phases: `--begin` snapshots the scope's current state and returns the working paths; the calling agent performs the semantic rewrite; `--commit` installs the result only if the live files are unchanged since `--begin`. turu SHALL NOT perform the semantic rewrite itself.

#### Scenario: Begin returns snapshot

- **WHEN** `turu distill repo --begin` runs
- **THEN** the envelope contains a revision id, per-file hashes, and the working paths
- **AND** the snapshot directory is created

#### Scenario: Commit installs distilled output

- **WHEN** the agent writes the distilled file and runs `turu distill repo --commit --revision <id>`
- **THEN** the distilled file atomically replaces the live file
- **AND** the envelope reports revision id, files replaced, and snapshot path

#### Scenario: Commit refuses on drift

- **WHEN** a live file changed after `--begin` (concurrent append)
- **THEN** `--commit` fails with a conflict error and a next-step suggestion
- **AND** no live file is modified

#### Scenario: Commit without a working file

- **WHEN** `--commit` runs and the agent never wrote the working path
- **THEN** the commit fails with an error naming the missing working file
- **AND** no live file is modified and the revision is retained

### Requirement: Immutable revisions

Pre-distill snapshots SHALL be retained: never deleted, never modified by later distill operations, and never pruned by default.

#### Scenario: Two consecutive distills

- **WHEN** distill `--begin`/`--commit` completes and a second distill cycle runs
- **THEN** two distinct revisions exist
- **AND** the first revision's content is byte-identical to its original state

#### Scenario: Commit replay

- **WHEN** the same committed revision is committed again with no further drift
- **THEN** the commit is a no-op reported in the envelope
