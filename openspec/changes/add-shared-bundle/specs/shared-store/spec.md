# shared-store — Spec Delta (proposed by add-shared-bundle)

## ADDED Requirements

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
