# shared-store — Spec Delta (proposed by add-repo-private-scope)

## ADDED Requirements

### Requirement: Bundles never carry the private zone

`turu bundle pack` SHALL exclude everything under the checkout's `.whisper/private/` tree from the produced bundle. `turu bundle unpack` SHALL never create, extend, or modify files under `.whisper/private/`. The exclusion SHALL apply to the exact top-level path only; a `private/` directory nested deeper under `.whisper/` is ordinary content.

#### Scenario: Pack with private content

- **WHEN** `turu bundle pack branch` runs in a checkout whose `.whisper/private/branches/main/notes.md` has entries
- **THEN** the bundle contains no path under `.whisper/private/`
- **AND** the envelope reports the excluded private entries count

#### Scenario: Unpack never writes into the private zone

- **WHEN** a bundle containing an entry that would target a path under `.whisper/private/` is unpacked
- **THEN** that entry is skipped and reported in the envelope
- **AND** no file under `.whisper/private/` is created or modified
