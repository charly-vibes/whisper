# Change: Repo-local publication with deterministic private zone

## Why

Resolves the strategic fork deferred by `add-shared-bundle` (beads `whisper-kt5`): branch-scoped notes die on one laptop because the knowledge store (`~/.whisper`) is machine-local, and the bundle primitive shipped without a transport. Decision: **transport is the repo's own git** (option A — committed `.whisper/`), with privacy enforced as a **structural exclusion zone** rather than judgment-based filtering. If private knowledge lived only in the global store, every future publication step would have to *decide* what is safe to sweep out and push — judgment is what turu exists to eliminate. With `.whisper/private/`, the boundary is a path, not a policy: no verb ever needs to judge.

## What Changes

- **Repo-local knowledge root**: `repo` and `branch` scopes resolve into `.whisper/` in the checkout (committed, travels with git) instead of the global store. The global store stays valid and readable — `turu recall` composes both, repo-local entries first.
- **Canonical private zone**: `.whisper/private/` (gitignored) mirrors the repo-local layout and is the destination for machine-specific or personal knowledge (`turu append --private`). Included in recall; **excluded from every publication/migration/transport verb by exact top-level path** — `bundle pack` never carries it, `consolidate` never moves it, nothing writes into it except private-scope operations.
- **Self-healing ignore rule**: `turu init`/`turu sync` ensure `.gitignore` contains `.whisper/private/` (append when absent, never duplicate, never touch other entries).
- **Privacy integrity in doctor**: (a) *effective* ignore verified via `git check-ignore`, (b) *no tracked files under `.whisper/private/`* (`git ls-files` — the real guard, since gitignore is not a hard guarantee), (c) advisory leak-shape lint on public repo-local knowledge files (machine paths, hostnames, token-shaped strings), per the incident-log-lint precedent (`whisper-4xl`).
- **Fork decision recorded**: B (bundles under git refs) and C (central remote store) rejected for now — recorded in this change's design.md; revisit only if commit noise actually hurts or a real team forms.

## Impact

- Affected specs: `workspace-routing` (MODIFIED + ADDED), `shared-store` (ADDED), `workspace-maintenance` (ADDED ×2)
- Affected code: `src/workspace.rs` (resolution, init), `src/bundle.rs` (exclusion), `src/doctor.rs` (privacy checks), managed-block template in `turu sync` (routing table)
- Depends on: `add-shared-bundle` archived (complete 12d ago, pending archive) — the `shared-store` delta builds on its proposed spec
- Breaking? No. Existing global-store entries remain readable; the default destination change for `repo`/`branch` scopes is the behavioral core, not a CLI break.
