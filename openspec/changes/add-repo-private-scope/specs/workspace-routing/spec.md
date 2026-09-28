# workspace-routing — Spec Delta (proposed by add-repo-private-scope)

## MODIFIED Requirements

### Requirement: Scope resolution is exact and total

Each scope SHALL resolve to exactly one path. `global` → `<workspace-root>/rules.md`; `worktree` → `<workspace-root>/repos/<repo-key>/worktrees/<slot>/env.md`; `group` → `<group-root>/repos/<repo-key>/env.md`. For `repo` and `branch`, the knowledge root SHALL be the checkout's repo-local `.whisper/` directory: `repo` → `.whisper/env.md`; `branch` → `.whisper/branches/<branch-slug>/notes.md`. The private destination mirrors the repo-local layout under `.whisper/private/` (`private/env.md`, `private/branches/<branch-slug>/notes.md`) and SHALL be reachable deterministically via the `--private` routing flag on append/resolve. The global-store equivalents (`repos/<repo-key>/env.md`, `repos/<repo-key>/branches/<branch-slug>/notes.md`) SHALL remain valid destinations via the explicit `--global` escape hatch. Resolution SHALL never guess or search.

#### Scenario: Branch scope resolves into the checkout

- **WHEN** `turu resolve branch` runs in a repo on branch `main`
- **THEN** the target is `<repo-root>/.whisper/branches/main/notes.md`

#### Scenario: Private routing flag

- **WHEN** `turu append branch --private --text ...` runs on branch `main`
- **THEN** the entry is written to `<repo-root>/.whisper/private/branches/main/notes.md`

#### Scenario: Global escape hatch

- **WHEN** `turu append branch --global --text ...` runs in a repo with key `github.com/org/repo`
- **THEN** the entry is written to `<workspace-root>/repos/github.com/org/repo/branches/main/notes.md`

#### Scenario: Group scope without an active group

- **WHEN** `turu resolve group` runs and no group is configured for this repo
- **THEN** the command fails with an error and a next-step suggestion
- **AND** no path is returned

#### Scenario: Unknown scope rejected

- **WHEN** `turu resolve nonsense` runs
- **THEN** the command fails with an error listing the valid scopes

## ADDED Requirements

### Requirement: Recall composes repo-local and global stores in store-major order

`turu recall` for `repo` and `branch` scopes SHALL serve entries from the checkout's repo-local root (including the private zone) and from the workspace root for the repo's key, in **store-major order**: all repo-local entries by recency first, then all workspace-root entries by recency, under the whole-entry byte budget. Entries SHALL be deduplicated by id across stores. Knowledge stored in the workspace root before this change SHALL remain servable.

#### Scenario: Pre-existing global-store knowledge stays servable

- **WHEN** an entry exists only in `<workspace-root>/repos/<repo-key>/env.md` and `turu recall repo` runs in the checkout
- **THEN** the entry is served from the global store

#### Scenario: Store-major composition order

- **WHEN** repo-local and workspace-root stores both contain entries for the repo scope and a workspace-root entry is more recent than some repo-local entries
- **THEN** all repo-local entries are served before any workspace-root entry
- **AND** within each store, entries are ordered by recency
- **AND** no entry appears twice

### Requirement: Private zone is read-included and publish-excluded

`.whisper/private/` SHALL be the canonical private zone of a repo workspace: included in recall composition, and excluded — by the exact top-level path `.whisper/private/` — from every publication and transport verb (`bundle pack`, `bundle unpack`, managed-block generation). No verb SHALL write into the private zone except explicit private-scope operations. `turu consolidate` operates on the workspace root only and structurally cannot reach the checkout; if it is ever extended to checkout-local knowledge, the private zone SHALL remain excluded from its moves and merges. The exclusion SHALL NOT apply to any other path, including a `private/` directory nested deeper under `.whisper/`.

#### Scenario: Recall serves private knowledge locally

- **WHEN** an entry exists under `.whisper/private/branches/main/notes.md` and `turu recall branch` runs on `main`
- **THEN** the entry is served

#### Scenario: Consolidate cannot reach the private zone

- **WHEN** `turu consolidate` runs against the workspace root
- **THEN** it scans only legacy repo-key variants under the workspace root
- **AND** if it is ever extended to checkout-local knowledge, no file under `.whisper/private/` is moved, merged, or removed
