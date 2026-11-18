# Project status brief

## Purpose / current state

none

## Focus

none

## In flight

- whisper-6p2 — docs: apply README front-door standard v0.4 (§5.1) (status: open; priority: 1; age: 2d)
- whisper-7u5 — Implement Hegel property tests (status: open; priority: 1; age: 2d; blocked by: whisper-cyl)
- whisper-cyl — Research: Hegel property-based testing plan (antithesis-research) (status: open; priority: 1; age: 2d)
- whisper-11e — openspec 1.x upgrade blocked: 4 validation failures under @fission-ai/openspec@1.14.2 (status: open; priority: 3; age: 33d)

## Blocked / waiting

- whisper-7u5 — Implement Hegel property tests (status: open; priority: 1; age: 2d; blocked by: whisper-cyl)

## Next / ready

- whisper-6p2 — docs: apply README front-door standard v0.4 (§5.1) (status: open; priority: 1; age: 2d)
- whisper-cyl — Research: Hegel property-based testing plan (antithesis-research) (status: open; priority: 1; age: 2d)
- whisper-11e — openspec 1.x upgrade blocked: 4 validation failures under @fission-ai/openspec@1.14.2 (status: open; priority: 3; age: 33d)

## Parked

none

### Needs re-triage

none

## Recent

- 475513d4ccf3ab65fe11cfd96b762f458c828da8 beads: close whisper-4z4 (2026-11-18T16:09:04Z)
- 34637d9ab08e9b38ac54671e798dc26418f59189 chore(gates): render the managed gate block from .ddl/config.toml (2026-11-18T16:02:35Z)
- 5753f45b2e94477cd4586a4b580167ae4774bfd0 ci: pin pretender 0.10.0 and drop the expired advisory lease (2026-11-18T15:45:19Z)
- 83925dadd1005ed7bc423e7598805c89ba56fd07 beads: file whisper-4z4 (pretender advisory lease expired in the CI smoke step) (2026-11-18T15:17:38Z)
- 1770002dd71a35418ece20051d0c096e4c1921d1 chore(openspec): migrate project.md into config.yaml; pin 2.1.0 (2026-11-18T15:01:12Z)
(113 more omitted)

## Health + confidence

Health: **amber** — basis: signals — owner: none — reason: orphan_commits: 18 commits in the 14-day window link no tracker id or open work-item path: 1770002dd71a35418ece20051d0c096e4c1921d1 (chore(openspec): migrate project.md into config.yaml; pin 2.1.0); 309d8fca784022598619e63ffbd3326057b3eddb (docs: apply issue-review fixes to README/release tickets); 34637d9ab08e9b38ac54671e798dc26418f59189 (chore(gates): render the managed gate block from .ddl/config.toml); 5753f45b2e94477cd4586a4b580167ae4774bfd0 (ci: pin pretender 0.10.0 and drop the expired advisory lease); 5ba4347a96f1b75b46f4a64f63033fe9545c762b (style: collapse nested if in hostname_assignment (clippy)); 69f9fe3086acf419530692b6cd81dd708906988a (style: prefix unused fixture binding with underscore (clippy)); 923983112bfc0fe5faa1e0dd535b19edc1e1c17b (refactor(quality): split tests/cli.rs into topic modules; fix all cli test debt); ab2db07ea3b0df57558b7494508dbaca79e96c55 (docs: file README front-door and release tickets); b2759ed5582ee30424dde8f5a5057debe9446187 (refactor(quality): split main.rs dispatch into cmds module (admin/knowledge/support)); b539acf9e14999e7c4c7198fbaf9b9dfe2a5803c (release: 0.11.2 — the binary reports the version the tag names); bc6f88661349aeb6b26c7416f9e8f2315f1d53ca (docs(brief): pretender debt fix brief — orchestrator artifact (docs-only; gate blocked by pre-existing debt)); cb5d9705be86ab9a71ca9ac06f8c5a62f9154b1a (refactor(quality): split doctor.rs into module dir (checks/content/leaks/managed/routing/tests)); cf0e410606de137ed4bcfd6e23cf732c9e0b7e66 (fix(lint): resolve clippy denies left over from module splits); df2672e74f591233352e72f7e30e83d3495d217e (refactor(quality): split oversized functions; move bundle/config tests to integration tests); e280c17288a94984e027f9ec9762df747e89c0ff (chore(openspec): adopt the 2.1 artifact layout); e472f206a075073b53815b7f5de74d2ef05dcc5d (refactor(quality): distill begin/commit + entry parsing splits; entry tests to integration file); ee4922f4bd9650d3844970913b6279154c7c21e0 (refactor(quality): clear unwrap/mut_ratio/function_lines debt in recall, bundle, config, skill_pack, usage); ef6954192ba36a8c73cfb64752cbf37da0ae5a9f (refactor(quality): split workspace.rs into module dir (block/init/resolve/scope/layout/tests)); spec_change_without_bead: openspec changes link no tracker id: `add-entry-model` changed 0 days ago
- signal: orphan commits — 18 commits in the 14-day window link no tracker id or open work-item path: 1770002dd71a35418ece20051d0c096e4c1921d1 (chore(openspec): migrate project.md into config.yaml; pin 2.1.0); 309d8fca784022598619e63ffbd3326057b3eddb (docs: apply issue-review fixes to README/release tickets); 34637d9ab08e9b38ac54671e798dc26418f59189 (chore(gates): render the managed gate block from .ddl/config.toml); 5753f45b2e94477cd4586a4b580167ae4774bfd0 (ci: pin pretender 0.10.0 and drop the expired advisory lease); 5ba4347a96f1b75b46f4a64f63033fe9545c762b (style: collapse nested if in hostname_assignment (clippy)); 69f9fe3086acf419530692b6cd81dd708906988a (style: prefix unused fixture binding with underscore (clippy)); 923983112bfc0fe5faa1e0dd535b19edc1e1c17b (refactor(quality): split tests/cli.rs into topic modules; fix all cli test debt); ab2db07ea3b0df57558b7494508dbaca79e96c55 (docs: file README front-door and release tickets); b2759ed5582ee30424dde8f5a5057debe9446187 (refactor(quality): split main.rs dispatch into cmds module (admin/knowledge/support)); b539acf9e14999e7c4c7198fbaf9b9dfe2a5803c (release: 0.11.2 — the binary reports the version the tag names); bc6f88661349aeb6b26c7416f9e8f2315f1d53ca (docs(brief): pretender debt fix brief — orchestrator artifact (docs-only; gate blocked by pre-existing debt)); cb5d9705be86ab9a71ca9ac06f8c5a62f9154b1a (refactor(quality): split doctor.rs into module dir (checks/content/leaks/managed/routing/tests)); cf0e410606de137ed4bcfd6e23cf732c9e0b7e66 (fix(lint): resolve clippy denies left over from module splits); df2672e74f591233352e72f7e30e83d3495d217e (refactor(quality): split oversized functions; move bundle/config tests to integration tests); e280c17288a94984e027f9ec9762df747e89c0ff (chore(openspec): adopt the 2.1 artifact layout); e472f206a075073b53815b7f5de74d2ef05dcc5d (refactor(quality): distill begin/commit + entry parsing splits; entry tests to integration file); ee4922f4bd9650d3844970913b6279154c7c21e0 (refactor(quality): clear unwrap/mut_ratio/function_lines debt in recall, bundle, config, skill_pack, usage); ef6954192ba36a8c73cfb64752cbf37da0ae5a9f (refactor(quality): split workspace.rs into module dir (block/init/resolve/scope/layout/tests))
- signal: spec change without bead — openspec changes link no tracker id: `add-entry-model` changed 0 days ago

## Sources & freshness

| source | status | observed_at | staleness | detail |
| --- | --- | --- | --- | --- |
| beads | present | 2026-11-18T16:06:56Z | fresh | 3 malformed line(s) skipped in .beads/issues.jsonl |
| git | present | 2026-11-18T16:09:04Z | fresh | none |
| openspec | present | 2026-11-18T14:49:17Z | fresh | none |
| whisper | present | 2026-10-15T14:38:02Z | stale | none |
| curation | present | 2026-11-18T21:12:09Z | fresh | none |

Curation: updated_at 2026-11-18T21:12:09Z; confirmed_at none.

Freshness: generated_at 2026-11-18T21:12:09Z; per-source observed_at and staleness are listed above.
