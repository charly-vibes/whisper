# add-shared-bundle Tasks

## 1. Pack (TDD: red→green) — depends on add-entry-model

- [x] 1.1 RED/GREEN: `turu bundle pack <scope>` emits a bundle (JSON envelope + files) keyed by canonical repo key; entries ordered by id
- [x] 1.2 RED/GREEN: packing identical workspace state twice yields byte-identical bundles (determinism test)

## 2. Unpack / merge (TDD: red→green)

- [x] 2.1 RED/GREEN: `turu bundle unpack` into a fresh workspace places files with identical entry ids (round-trip test)
- [x] 2.2 RED/GREEN: unpack into an existing workspace extends without duplicating lines; entry ids already present are skipped and reported (`duplicates_skipped`)
- [x] 2.3 RED/GREEN: unpack never overwrites existing file content (extend-only, mirroring consolidate)

## 3. Validation

- [x] 3.1 `just test` green; `just lint` clean
- [x] 3.2 Doctor reports bundle capability + workspace paths involved
- [x] 3.3 README: bundle is transport-agnostic; strategic fork (in-repo vs git-refs vs remote) documented as open decision
