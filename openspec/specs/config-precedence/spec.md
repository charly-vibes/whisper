# config-precedence Specification

## Purpose

Baseline spec for existing behavior: layered configuration — global (`~/.config/whisper/config.toml`, or `$XDG_CONFIG_HOME`), group workspaces, and private per-repo overrides. Precedence: **repo > group > global**.

## Requirements

### Requirement: Layered resolution with repo > group > global

Configuration SHALL resolve in layers: the global file provides the default workspace root and named `[groups.*]` workspaces; a group routes a set of repos into one shared knowledge directory; the private repo config (`.whisper/config.toml`, found by walking up from the working directory) may join a group or override the workspace root. Repo overrides SHALL win over global defaults.

#### Scenario: Repo joins a group

- **WHEN** the repo config sets `group = "team"` and the global config defines `[groups.team]` with a root
- **THEN** repo, branch, and worktree scopes resolve under the group root
- **AND** the `global` scope still resolves to the workspace root

#### Scenario: Repo overrides the workspace root

- **WHEN** the repo config sets `workspace_root` and no group is active
- **THEN** all scopes resolve under that private root

### Requirement: Repo config discovery walks up

The repo config SHALL be discovered by walking up **from the working directory** to the first `.whisper/config.toml`; files above that first hit are not consulted for repo overrides.

#### Scenario: Config found in a parent

- **WHEN** the working directory is `<repo>/src/module` and `.whisper/config.toml` exists at `<repo>/`
- **THEN** that config is applied

#### Scenario: Nested config shadows the repo root

- **WHEN** `.whisper/config.toml` exists both in the current working directory's subtree root and at the repo root
- **THEN** the first one found walking up from the working directory wins
- **AND** the repo root's config is not consulted for that invocation

### Requirement: Global default root

With no repo config and no group membership, the workspace root SHALL default to `~/.whisper` (honoring a `workspace_root` set in the global config, with `~` expansion).

#### Scenario: Fresh machine

- **WHEN** no config files exist anywhere
- **THEN** scopes resolve under `~/.whisper`
