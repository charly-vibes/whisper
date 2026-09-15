# entry-model — Spec Delta (proposed by add-entry-model)

## ADDED Requirements

### Requirement: Deterministic entry identity

Every appended entry SHALL receive a deterministic id derived from scope, timestamp, and content (sha2), and appending an entry whose id already exists in that scope SHALL be a no-op reported as `duplicate` in the envelope.

#### Scenario: Idempotent append

- **WHEN** the same text is appended twice to the same scope in the same second (concurrent agents)
- **THEN** the second append does not modify the file
- **AND** the envelope reports `duplicate: true`
- **AND** the file contains exactly one copy of the entry

#### Scenario: Re-statement later is a new entry

- **WHEN** the same text is appended at a later timestamp
- **THEN** a new entry with a new id is created (later distillation resolves semantic duplication)

### Requirement: Human-readable structured entries

Entries SHALL be stored as structured markdown carrying id, timestamp, topic key, and optional `supersedes` reference, such that the scope file remains readable and valid markdown without turu.

#### Scenario: File remains readable markdown

- **WHEN** entries are appended with `turu append --topic infra`
- **THEN** the scope file is still valid markdown readable without turu
- **AND** each entry line carries its id, timestamp, and topic in-band

#### Scenario: Legacy freeform lines tolerated

- **WHEN** a scope file contains pre-existing freeform lines without ids
- **THEN** append, recall, and distill operations succeed without modifying those lines
- **AND** `turu doctor` reports them as unmanaged (warning, not error)

### Requirement: Canonical timestamps gate entry parsing

A ledger line SHALL parse as an entry only when its timestamp is canonical RFC-3339 seconds UTC; lines with malformed, sub-second, or offset-form timestamps SHALL be treated as unmanaged freeform so they cannot corrupt recency ordering.

#### Scenario: Malformed timestamp degrades to freeform

- **WHEN** a scope file contains `- banana [id:abc] text` or a sub-second timestamp line
- **THEN** recall serves it verbatim as freeform, not as a ranked entry
- **AND** `turu doctor` counts it as unmanaged content

### Requirement: Leading whitespace normalized at ingest

Appended text SHALL have per-line leading whitespace stripped at ingest, and the entry id SHALL be computed over the normalized text, so any leading-indented content round-trips through the exact-two-space continuation convention.

#### Scenario: Indented multi-line text round-trips

- **WHEN** multi-line text with indented lines is appended
- **THEN** the stored entry carries the normalized text (leading whitespace stripped per line)
- **AND** re-appending the same raw text is reported as `duplicate` (same id)

### Requirement: Mechanical supersede marking

turu SHALL record a `supersedes` reference between entries without interpreting entry content.

#### Scenario: Supersede an existing entry

- **WHEN** `turu append --supersedes <id>` is run with a valid target id
- **THEN** the new entry carries the reference
- **AND** the target entry is marked superseded

#### Scenario: Supersede an unknown id

- **WHEN** `--supersedes` names an id not present in the scope
- **THEN** the command fails via the envelope error path with a next-step suggestion
- **AND** the scope file is unchanged
