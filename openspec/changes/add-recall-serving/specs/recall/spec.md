# recall — Spec Delta (proposed by add-recall-serving)

## ADDED Requirements

### Requirement: Scoped slice serving

`turu recall <scope>` SHALL serve entries from one or more scopes ranked by recency (newest first), filtered by topic, and bounded by a byte budget, through the genesis envelope. `<scope>` SHALL accept the five routing scopes plus `all` — a composition of every applicable scope in precedence order; `all` is a recall argument, not a routing scope.

#### Scenario: Budget-bounded recall

- **WHEN** `turu recall branch --budget 400` is run against entries exceeding 400 bytes
- **THEN** the envelope contains only the newest entries that fit the budget
- **AND** the envelope reports `served_bytes` and `entries_skipped`

#### Scenario: Topic filter

- **WHEN** `turu recall branch --topic infra` is run
- **THEN** only entries whose topic key is `infra` are served

#### Scenario: All-scope composition

- **WHEN** `turu recall all` is run
- **THEN** entries from every applicable scope are served in precedence order
- **AND** the routing `Scope` enum is unchanged (`all` is not added to it)

#### Scenario: Freeform lines pass through unranked

- **WHEN** the scope file contains legacy freeform lines without ids
- **THEN** they are served verbatim after all ranked entries

### Requirement: Superseded entries excluded by default

Recall SHALL exclude superseded entries unless explicitly opted in.

#### Scenario: Supersede excludes from recall

- **WHEN** entry B supersedes entry A and recall runs without flags
- **THEN** A is not served
- **AND** `turu recall --include-superseded` serves both

### Requirement: Whole-entry budget atomicity

The byte budget SHALL apply to whole entries: an entry that does not fit is skipped in its entirety, never truncated.

#### Scenario: Entry straddling the budget

- **WHEN** the next-ranked entry does not fit the remaining budget
- **THEN** it is skipped whole and counted in `entries_skipped`
- **AND** no partial entry text is served

### Requirement: Context-horizon boundary reporting

Recall output SHALL report the boundary explicitly — bytes served and budget unused — so the calling agent can decide to skip loading when the content fits in context.

#### Scenario: Recall below the horizon

- **WHEN** the budget exceeds the scope's total content
- **THEN** all entries are served
- **AND** `budget_unused > 0` is reported in the envelope

### Requirement: Deterministic precedence across scopes

When recall serves entries from multiple scopes, the resolution order SHALL be global < group < repo < branch < worktree — the same precedence as `turu resolve` — and each served entry SHALL carry its scope.

#### Scenario: Precedence is reported, never guessed

- **WHEN** a global entry and a branch entry share a topic and both are served
- **THEN** each entry in the envelope names its scope
- **AND** the order matches the documented precedence
