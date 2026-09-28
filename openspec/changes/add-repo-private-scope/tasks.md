# add-repo-private-scope Tasks

Depends on: `add-shared-bundle` archived (its `shared-store` spec must exist in `specs/` first).

## 1. Repo-local knowledge root (TDD: red→green)

- [x] 1.1 RED/GREEN: `repo` and `branch` scopes resolve into the checkout's `.whisper/` (`env.md`, `branches/<slug>/notes.md`); `global`, `group`, `worktree` scopes unchanged (whisper-6fv, 8a83802)
- [x] 1.2 RED/GREEN: `turu recall` composes repo-local root (incl. private zone) + workspace root in **store-major order** — all repo-local entries by recency, then workspace-root entries by recency, whole-entry byte budget, id dedup across stores; interleaving test where a newer workspace-root entry still serves *after* older repo-local entries (whisper-6fv; private zone lands with 2.x)
- [x] 1.3 RED/GREEN: `turu append` for `repo`/`branch` lands in the repo-local root; explicit `--global` escape hatch still writes the global-store path (whisper-6fv)

## 2. Private zone (TDD: red→green)

- [ ] 2.1 RED/GREEN: `turu append --private` routes into `.whisper/private/` mirroring the repo-local layout (`private/env.md`, `private/branches/<slug>/notes.md`)
- [ ] 2.2 RED/GREEN: recall includes the private zone (private + repo-local + global composition)
- [ ] 2.3 RED/GREEN: `bundle pack` never includes anything under `.whisper/private/` (exact-path exclusion; a `private/` nested elsewhere is NOT excluded)
- [ ] 2.4 RED/GREEN: `bundle unpack` never creates files under `.whisper/private/`
- [ ] 2.5 RED/GREEN: `turu bundle pack` refuses any scope resolving under `.whisper/private/` — clear error, no bundle produced (unit-level guard binding future routing changes)
- [ ] 2.6 RED/GREEN: consolidate regression guard — legacy-variant detection operates only on the workspace root; no checkout path (incl. `.whisper/private/`) is ever an input to a move or merge

## 3. Ignore rule + doctor (TDD: red→green)

- [ ] 3.1 RED/GREEN: `turu init`/`turu sync` ensure `.gitignore` contains `.whisper/private/` — append when absent, never duplicate, never modify other entries
- [ ] 3.2 RED/GREEN: doctor privacy checks — (a) effective ignore verified via `git check-ignore` (warning + fix hint when not ignored — text presence is not the truth), (b) no tracked files under `.whisper/private/` via `git ls-files` (prominent failure), (c) advisory leak-shape lint on public repo-local knowledge files (machine paths, hostnames, token-shaped strings) via envelope `warnings[]`
- [ ] 3.3 Update doctor pass-count tests for the new checks (known shift, cf. t6j gotcha)

## 4. Contract surfaces

- [x] 4.1 `turu sync` managed-block routing table shows repo-local root + private zone destinations (whisper-6fv: checkout + `--global` destinations shown; private-zone line lands with whisper-eiy)
- [ ] 4.2 README + whisper skill pack: transport decision (A + private zone); "git history is forever — prevention only" stated verbatim; "repo-local publication assumes a pushable checkout — `--global` is the fallback for read-only checkouts (CI, no-push contributors)"
- [ ] 4.3 `just ci` green (fmt + clippy `-D warnings` + test + build-locked)
