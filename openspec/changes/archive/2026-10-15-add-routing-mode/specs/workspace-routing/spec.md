# workspace-routing delta

## ADDED Requirements

### Requirement: Declarative per-repo routing mode

`<repo>/.whisper/config.toml` SHALL accept a `routing` key with value
`repo-local` or `machine-local`, defaulting to `repo-local`. The mode
SHALL be the persistent default destination for the repo and branch
scopes: `repo-local` resolves them into the checkout's committed
`.whisper/` zone (decision A); `machine-local` resolves them into the
machine-local store — the persistent equivalent of the `--global`
escape hatch. Per-invocation overrides compose on top: `--private`
keeps routing into the checkout's private zone and group routing keeps
priority when a group is active. An unknown mode value SHALL fail the
invocation with an error naming the valid values.

#### Scenario: Machine-local routing is the persistent default

- **WHEN** `.whisper/config.toml` sets `routing = "machine-local"` and `turu append branch --text ...` runs
- **THEN** the entry is written to the machine-local store without any flag

#### Scenario: Repo-local remains the default without a routing key

- **WHEN** the repo config has no `routing` key
- **THEN** repo/branch scopes resolve into the checkout's committed zone (decision A)

#### Scenario: Unknown routing mode is rejected with the valid values

- **WHEN** the repo config sets `routing = "cloud"`
- **THEN** the invocation fails with an error naming `repo-local` and `machine-local`

#### Scenario: The private zone override composes over the declared mode

- **WHEN** `turu append branch --private --text ...` runs under `machine-local` routing
- **THEN** the entry is written to `<repo-root>/.whisper/private/branches/main/notes.md`

### Requirement: Init provisions the declared routing layer

`turu init` SHALL provision exactly the layer the declared routing mode
resolves to for repo/branch scopes: both checkout and store layers under
`repo-local` (recall composes both), only the store layer under
`machine-local` — never both provisioning passes for the same scope.

#### Scenario: Init under machine-local routing provisions only the store

- **WHEN** `turu init` runs with `routing = "machine-local"`
- **THEN** the store slots for repo/branch are created
- **AND** no checkout repo/branch slot files are created

### Requirement: Doctor reports the declared routing mode

`turu doctor` SHALL report the declared routing mode and the effective
repo/branch route via a check, and SHALL warn when `machine-local`
routing is active while the checkout layer still carries content (the
mode switch is a config edit; leftovers must be surfaced, not silent).

#### Scenario: Doctor flags stale checkout-layer content under machine-local

- **WHEN** `turu doctor` runs with `routing = "machine-local"` and the checkout's repo slot file has content
- **THEN** the routing check warns naming the stale files with a migration hint
- **AND** a clean switch reports the store route and no warning
