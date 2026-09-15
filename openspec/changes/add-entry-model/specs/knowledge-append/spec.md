# knowledge-append — Spec Delta (proposed by add-entry-model)

## MODIFIED Requirements

### Requirement: Append creates then extends

`turu append <scope>` SHALL create the scope's file (with parent directories) if missing, and otherwise rewrite it in place with the new entry appended last. Existing content SHALL never be lost: entry lines are re-rendered canonically and unmanaged freeform lines are preserved verbatim in their original order.

#### Scenario: First append creates the file

- **WHEN** the scope's file does not exist
- **THEN** `turu append --text "first fact"` creates it containing one structured entry line
- **AND** the envelope reports the target `path`, `appended_bytes`, the entry `id`, and `duplicate: false`

#### Scenario: Subsequent appends extend

- **WHEN** the scope's file already contains content
- **THEN** the new entry is appended after the existing content
- **AND** freeform lines and prior entries remain present, in order

## ADDED Requirements

### Requirement: Topic keys are validated mechanically

`turu append --topic` SHALL reject topics containing whitespace, parentheses, or `#`, since those characters carry structural meaning in the ledger line format.

#### Scenario: Invalid topic rejected

- **WHEN** `turu append --topic "two words"` runs
- **THEN** the command fails with an error and a next-step suggestion
- **AND** the scope file is unchanged

## REMOVED Requirements

None.
