# workspace-maintenance Specification

## Purpose

Baseline spec for existing behavior: workspace lifecycle commands — `init` (never-overwriting layout creation), `consolidate` (legacy repo-key migration), and read-only diagnostics (`status`, `check`, `doctor`).

## Requirements

### Requirement: Init creates the layout without overwriting

`turu init` SHALL create the checkout's workspace slot: `rules.md`, the repo slot (`env.md`), the branch slot (`notes.md`, `context.md`, `plan.md`), and the worktree slot (`env.md`). Existing files SHALL be reported as existing and left untouched.

#### Scenario: Fresh init

- **WHEN** `turu init` runs against a new checkout
- **THEN** all slot files are created and reported as created

#### Scenario: Idempotent re-init

- **WHEN** `turu init` runs again after a previous init
- **THEN** no file is modified or overwritten
- **AND** all files are reported as existing

### Requirement: Consolidate migrates legacy repo-key dirs non-destructively

`turu consolidate` SHALL migrate legacy repo-key directory variants (bare repo name, `host:name` colon form matching the bare name) into the canonical key dir: rename the whole dir when the canonical dir is absent; otherwise move entries without conflict or merge text files by extending with non-duplicate lines. Colon dirs that do not match this repo's bare name SHALL never be touched.

#### Scenario: Canonical dir absent

- **WHEN** a legacy variant exists and the canonical dir does not
- **THEN** the legacy dir is renamed to the canonical dir
- **AND** the envelope reports it as moved

#### Scenario: Merge extends without duplicating

- **WHEN** a legacy variant exists alongside the canonical dir and both contain overlapping lines in a text file
- **THEN** the merged file contains each line exactly once
- **AND** the legacy dir is removed afterwards

#### Scenario: Unrelated colon dirs are safe

- **WHEN** `repos/` contains a colon-form dir belonging to a different repo
- **THEN** `turu consolidate` leaves it untouched

### Requirement: Diagnostics are read-only

`turu status`, `turu check`, and `turu doctor` SHALL report workspace state (paths, existence flags, legacy variants, group definitions, missing files, managed-block state) without modifying any file.

#### Scenario: Status is side-effect free

- **WHEN** `turu status` runs
- **THEN** the workspace layout is unchanged afterwards

#### Scenario: Check flags legacy variants

- **WHEN** legacy repo-key dirs exist
- **THEN** `turu check` reports them and suggests `turu consolidate`
