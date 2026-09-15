# add-distill-contract Tasks

## 1. Revision snapshot (TDD: red→green) — depends on add-entry-model

- [x] 1.1 RED/GREEN: `turu distill <scope> --begin` copies scope files to an immutable revision dir and returns revision id, per-file hashes, working paths in the envelope
- [x] 1.2 RED/GREEN: two consecutive `--begin` runs produce two distinct revisions; neither is deleted or modified by later runs

## 2. Commit with conflict detection (TDD: red→green)

- [x] 2.1 RED/GREEN: agent writes distilled output to the working path; `turu distill <scope> --commit --revision <id>` atomically replaces the live file and reports revision id, files replaced, snapshot path
- [x] 2.2 RED/GREEN: live file changed since `--begin` (e.g. concurrent append) → commit fails with conflict error + next-step suggestion; live file untouched
- [x] 2.3 RED/GREEN: committing a revision twice with no drift is a no-op reported in the envelope
- [x] 2.4 RED/GREEN: `--commit` with no working file written fails (error names the missing path); nothing installed, revision retained

## 3. Freeform passthrough (TDD: red→green)

- [x] 3.1 RED/GREEN: distill on a scope containing legacy freeform lines works: they are included in the snapshot and the agent may rewrite them, but turu imposes no format requirement

## 4. Validation

- [x] 4.1 `just test` green; `just lint` clean
- [x] 4.2 Doctor reports pending distill revisions (begun but not committed) as informational
- [x] 4.3 `turu sync` managed block documents the distill contract; whisper skill pack updated to route the judgment half through the skill
