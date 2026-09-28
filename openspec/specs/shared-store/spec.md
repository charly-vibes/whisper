# shared-store Specification

## Purpose
TBD - created by archiving change add-shared-bundle. Update Purpose after archive.
## Requirements
### Requirement: Deterministic knowledge bundle

`turu bundle pack <scope>` SHALL produce a single archive keyed by the canonical repo key containing the scope's entries with ids, ordered deterministically such that packing identical state yields identical bytes.

#### Scenario: Deterministic pack

- **WHEN** the same workspace state is packed twice
- **THEN** the two bundles are byte-identical

#### Scenario: Round trip into a fresh workspace

- **WHEN** a bundle is packed and then unpacked into a fresh workspace
- **THEN** entries are present with identical ids

### Requirement: Non-destructive unpack

`turu bundle unpack` SHALL merge using extend-without-duplicate rules (mirroring `turu consolidate`), skip entries whose ids already exist locally, and never overwrite existing file content.

#### Scenario: Merge into an existing workspace

- **WHEN** a bundle is unpacked into a workspace that already has some of the entries
- **THEN** existing entries are skipped and reported as `duplicates_skipped`
- **AND** existing file content is extended, not overwritten

### Requirement: Transport-agnostic bundles

The bundle format SHALL make no assumption about transport; how bundles travel between workspaces is out of scope for the bundle capability.

#### Scenario: Bundle carries no transport state

- **WHEN** a bundle is produced
- **THEN** it contains only knowledge content and addressing (repo key, scope, entry ids)
- **AND** no transport-specific fields are required to unpack it

#### Scenario: Pack group scope with no active group

- **WHEN** `turu bundle pack group` runs while no group is active for this repo
- **THEN** the command fails with the same error and suggestion as `turu resolve group`

### Requirement: Bundles never carry the private zone

`turu bundle pack` SHALL exclude everything under the checkout's `.whisper/private/` tree from the produced bundle, and SHALL refuse — with a clear error and no bundle produced — any scope whose resolved path lies under `.whisper/private/` (a guard binding future routing changes; no shipped scope resolves there today). `turu bundle unpack` SHALL never create, extend, or modify files under `.whisper/private/`, and SHALL place entries at the paths resolved under current resolution rules — a bundle packed before this change unpacks into the repo-local root. The exclusion SHALL apply to the exact top-level path only; a `private/` directory nested deeper under `.whisper/` is ordinary content.

#### Scenario: Pack refuses private-resolved scopes

- **WHEN** `turu bundle pack` is requested for a scope whose resolved path lies under `.whisper/private/`
- **THEN** the command fails with an error explaining the private zone is never transported
- **AND** no bundle is produced

#### Scenario: Pack with private content

- **WHEN** `turu bundle pack branch` runs in a checkout whose `.whisper/private/branches/main/notes.md` has entries
- **THEN** the bundle contains no path under `.whisper/private/`
- **AND** the envelope reports the excluded private entries count

#### Scenario: Unpack never writes into the private zone

- **WHEN** a bundle containing an entry that would target a path under `.whisper/private/` is unpacked
- **THEN** that entry is skipped and reported in the envelope
- **AND** no file under `.whisper/private/` is created or modified

#### Scenario: Legacy bundle unpacks into current resolution

- **WHEN** a bundle packed before this change (entries addressed to workspace-root paths) is unpacked in a checkout
- **THEN** entries are placed at the paths resolved under current rules
- **AND** existing-content rules (extend, never overwrite) are unchanged

