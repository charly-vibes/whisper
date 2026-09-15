# managed-block Specification

## Purpose

Baseline spec for existing behavior: the `<!-- TURU:START -->` managed block injected into the agent-facing file — the contract that lets agents read deterministic routing paths from a file instead of re-deriving them.

## Requirements

### Requirement: Inject or refresh the managed block

`turu sync` SHALL inject the `<!-- TURU:START -->` / `<!-- TURU:END -->` block into `AGENTS.md` at the repo root (or the `--file` override), containing the workspace root, repo key, branch slug, worktree slot, and the deterministic routing map for all scopes. Content outside the block SHALL never be modified.

#### Scenario: First sync creates the block

- **WHEN** the target file exists without a managed block
- **THEN** the block is injected and the envelope reports the action
- **AND** existing file content is preserved

#### Scenario: Target file missing

- **WHEN** the target file does not exist
- **THEN** `turu sync` creates it containing the managed block
- **AND** the envelope reports the action as created

#### Scenario: Re-sync updates in place

- **WHEN** the target file already contains a managed block and routing facts changed (e.g. new branch)
- **THEN** only the block content is replaced
- **AND** content outside the block is byte-identical to before

#### Scenario: File override

- **WHEN** `turu sync --file CLAUDE.md` runs
- **THEN** the block is injected into that file instead of `AGENTS.md`

### Requirement: Block content is generated, never hand-edited

The managed block SHALL be regenerated only by `turu sync`; its content is derived entirely from checkout facts and resolved config.

#### Scenario: Content matches resolve

- **WHEN** the managed block lists a scope path
- **THEN** it is identical to the path `turu resolve <scope>` returns for the same checkout
