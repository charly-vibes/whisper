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
- **THEN** `turu check` reports them together with the canonical key to route new knowledge into

### Requirement: Init and sync ensure the private-zone ignore rule

`turu init` and `turu sync` SHALL ensure the checkout's `.whisper/private/` is *effectively* ignored: appending the rule `.whisper/private/` to the root `.gitignore` when `git check-ignore` does not already report the path as ignored (e.g. when covered by a nested rule), never duplicating an existing rule, and never modifying or removing any other entry.

#### Scenario: Fresh init adds the rule

- **WHEN** `turu init` runs in a checkout whose `.gitignore` lacks `.whisper/private/`
- **THEN** the rule is appended exactly once
- **AND** the envelope reports it as created

#### Scenario: Idempotent re-init

- **WHEN** `turu init` runs again after the rule was added
- **THEN** no duplicate rule is appended
- **AND** existing `.gitignore` content is unchanged

### Requirement: Doctor reports privacy integrity

`turu doctor` SHALL run three privacy checks and report them read-only via the envelope: (a) `.whisper/private/` is effectively ignored — verified with `git check-ignore`, not by text presence in a `.gitignore` file (warning with a fix hint when not ignored); (b) no tracked files exist under `.whisper/private/` (`git ls-files` — a prominent failure, since gitignore is not a hard guarantee and committed history cannot be retracted); (c) an advisory leak-shape lint on public repo-local knowledge files flagging machine-specific paths, hostnames, and token-shaped strings — reported via envelope `warnings[]`, never blocking and never modifying files.

#### Scenario: Private zone not effectively ignored

- **WHEN** `turu doctor` runs in a checkout where `git check-ignore .whisper/private/` does not report the path (rule absent, or present only in a nested `.gitignore` whose scope does not cover it)
- **THEN** the check reports a warning with the fix hint (`turu init` or `turu sync`)

#### Scenario: Tracked file under the private zone

- **WHEN** `.whisper/private/env.md` is tracked by git (committed before the ignore rule, or force-added)
- **THEN** the doctor check fails prominently, stating that the file is published in history and prevention cannot be retroactive

#### Scenario: Leak-shaped content in public files

- **WHEN** a public repo-local knowledge file contains machine-specific paths such as `/home/<user>/` or token-shaped strings
- **THEN** doctor emits an advisory warning naming the file and match shape
- **AND** the command still exits successfully

