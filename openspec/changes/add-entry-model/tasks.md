# add-entry-model Tasks

## 1. Entry identity (TDD: red→green)

- [ ] 1.1 RED: failing test — `turu append --text "fact"` twice to the same scope creates exactly one copy of the entry; second envelope reports `duplicate: true`
- [ ] 1.2 GREEN: derive entry id as sha2(scope key + timestamp + content) in `Target::append`; skip write on id collision
- [ ] 1.3 RED/GREEN: test that the id appears in the envelope and is stable across repeated appends of identical content at different times (timestamp in hash input → different id; document chosen inputs in design.md)

## 2. Entry format (TDD: red→green)

- [ ] 2.1 RED/GREEN: appended entry is a structured markdown line/block carrying id, timestamp, topic; file remains valid markdown readable without turu
- [ ] 2.2 RED/GREEN: `--topic` flag on `turu append`; entries without `--topic` get an empty topic key (backwards compatible)
- [ ] 2.3 REFACTOR (separate ticket): extract entry parse/serialize into its own module (`src/entry.rs`) — no behavior change

## 3. Supersede marking (TDD: red→green)

- [ ] 3.1 RED/GREEN: `turu append --supersedes <id>` records the reference on the new entry and marks the target entry superseded; turu does not interpret content
- [ ] 3.2 RED/GREEN: superseding an unknown id fails with envelope error + next-step suggestion

## 4. Validation

- [ ] 4.1 `just test` green; `just lint` clean
- [ ] 4.2 `turu doctor` extended to flag malformed entry lines (non-blocking warning)
- [ ] 4.3 Update the AGENTS.md managed block command list (`turu sync`)
