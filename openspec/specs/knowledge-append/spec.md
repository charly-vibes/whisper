# knowledge-append Specification

## Purpose

Baseline spec for existing behavior: `turu append` — extending a scope's knowledge file without duplication or clobbering. (Structured entry ids, idempotency, and supersede marks are proposed separately in `add-entry-model`.)

## Requirements

### Requirement: Append creates then extends

`turu append <scope>` SHALL create the scope's file (with parent directories) if missing, and otherwise append to it in place. Existing content SHALL never be rewritten or truncated.

#### Scenario: First append creates the file

- **WHEN** the scope's file does not exist
- **THEN** `turu append --text "first fact"` creates it containing the text
- **AND** the envelope reports the target `path` and `appended_bytes`

#### Scenario: Subsequent appends extend

- **WHEN** the scope's file already contains content
- **THEN** `turu append --text "second fact"` adds the text after the existing content
- **AND** existing content is byte-identical to before

#### Scenario: Empty input rejected

- **WHEN** `turu append` runs with blank `--text` or empty stdin
- **THEN** the command fails with an error and a next-step suggestion
- **AND** the scope file is unchanged

### Requirement: Newline normalization

Appended text SHALL be trimmed of trailing whitespace and written with exactly one trailing newline.

#### Scenario: Trailing whitespace collapse

- **WHEN** `turu append --text "fact\n\n"` runs
- **THEN** the file ends with `fact` and a single newline

### Requirement: Stdin input

`turu append` SHALL accept text from stdin via `--stdin` as an alternative to repeatable `--text`.

#### Scenario: Piped input

- **WHEN** text is piped to `turu append repo --stdin`
- **THEN** the piped text is appended exactly as `--text` would append it
