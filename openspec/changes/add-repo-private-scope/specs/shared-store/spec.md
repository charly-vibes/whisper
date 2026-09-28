# shared-store — Spec Delta (proposed by add-repo-private-scope)

## ADDED Requirements

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
