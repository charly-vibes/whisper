# workspace-maintenance — Spec Delta (proposed by add-repo-private-scope)

## ADDED Requirements

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
