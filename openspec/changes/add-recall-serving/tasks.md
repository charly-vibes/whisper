# add-recall-serving Tasks

## 1. Slice serving (TDD: red→green) — depends on add-entry-model

- [ ] 1.1 RED/GREEN: `turu recall branch` returns the scope's entries ranked by recency (newest first), each with id, timestamp, topic, precedence
- [ ] 1.2 RED/GREEN: superseded entries are excluded by default; `--include-superseded` opts back in
- [ ] 1.3 RED/GREEN: `--topic <key>` filters to entries whose topic matches; unmanaged freeform lines pass through unranked at the end

## 2. Budget boundary (TDD: red→green)

- [ ] 2.1 RED/GREEN: `--budget <bytes>` bounds the served slice to the newest entries that fit; envelope reports `served_bytes`, `budget_unused`, `entries_skipped`
- [ ] 2.2 RED/GREEN: budget exceeding available content serves everything with `budget_unused > 0` (the "task fits in context" signal — agent may skip loading)

## 3. Cross-scope precedence (TDD: red→green)

- [ ] 3.1 RED/GREEN: `turu recall all` (or per-scope composition) serves entries in deterministic precedence global < group < repo < branch < worktree; each entry carries its scope
- [ ] 3.2 RED/GREEN: precedence order matches config precedence already used by `turu resolve`

## 4. Validation

- [ ] 4.1 `just test` green; `just lint` clean
- [ ] 4.2 `turu sync` managed block documents `turu recall` as the read path
- [ ] 4.3 Envelope examples added to README (`--json` is the machine contract)
