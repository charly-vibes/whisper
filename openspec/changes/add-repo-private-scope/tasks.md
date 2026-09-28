# add-repo-private-scope Tasks

Depends on: `add-shared-bundle` archived (its `shared-store` spec must exist in `specs/` first).

## 1. Repo-local knowledge root (TDD: red→green)

- [ ] 1.1 RED/GREEN: `repo` and `branch` scopes resolve into the checkout's `.whisper/` (`env.md`, `branches/<slug>/notes.md`); `global`, `group`, `worktree` scopes unchanged
- [ ] 1.2 RED/GREEN: `turu recall` composes repo-local + global stores for the repo's key, repo-local entries served first, entry-id dedup across stores
- [ ] 1.3 RED/GREEN: `turu append` for `repo`/`branch` lands in the repo-local root; explicit `--global` escape hatch still writes the global-store path

## 2. Private zone (TDD: red→green)

- [ ] 2.1 RED/GREEN: `turu append --private` routes into `.whisper/private/` mirroring the repo-local layout (`private/env.md`, `private/branches/<slug>/notes.md`)
- [ ] 2.2 RED/GREEN: recall includes the private zone (private + repo-local + global composition)
- [ ] 2.3 RED/GREEN: `bundle pack` never includes anything under `.whisper/private/` (exact-path exclusion; a `private/` nested elsewhere is NOT excluded)
- [ ] 2.4 RED/GREEN: `bundle unpack` never creates files under `.whisper/private/`
- [ ] 2.5 RED/GREEN: `consolidate` never moves or merges anything under `.whisper/private/` (moves-data precedent: this detection cannot be loose)

## 3. Ignore rule + doctor (TDD: red→green)

- [ ] 3.1 RED/GREEN: `turu init`/`turu sync` ensure `.gitignore` contains `.whisper/private/` — append when absent, never duplicate, never modify other entries
- [ ] 3.2 RED/GREEN: doctor privacy checks — (a) ignore rule present (warning + fix hint if missing), (b) no tracked files under `.whisper/private/` via `git ls-files` (prominent failure), (c) advisory leak-shape lint on public repo-local knowledge files (machine paths, hostnames, token-shaped strings) via envelope `warnings[]`
- [ ] 3.3 Update doctor pass-count tests for the new checks (known shift, cf. t6j gotcha)

## 4. Contract surfaces

- [ ] 4.1 `turu sync` managed-block routing table shows repo-local root + private zone destinations
- [ ] 4.2 README + whisper skill pack: transport decision (A + private zone), "git history is forever — prevention only" stated verbatim
- [ ] 4.3 `just ci` green (fmt + clippy `-D warnings` + test + build-locked)
