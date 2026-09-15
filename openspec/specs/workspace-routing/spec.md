# workspace-routing Specification

## Purpose

Baseline spec for existing behavior: canonical repo-key derivation, branch slugs, worktree slots, and scope resolution — the deterministic addressing that makes knowledge routing a pure function of the checkout, never a guess.

## Requirements

### Requirement: Canonical repo key is a pure function of the remote URL

The repo key SHALL be derived only from the `origin` remote URL: scheme and `git@` prefixes stripped, first colon replaced with a slash, trailing `.git` and slashes removed (`git@host:org/repo.git` → `host/org/repo`). The derivation SHALL NOT depend on machine identity.

#### Scenario: SSH remote

- **WHEN** origin is `git@github.com:org/repo.git`
- **THEN** the repo key is `github.com/org/repo`

#### Scenario: HTTPS remote

- **WHEN** origin is `https://host/org/repo.git`
- **THEN** the repo key is `host/org/repo`

#### Scenario: No remote

- **WHEN** the checkout has no origin remote
- **THEN** the repo key is `local/<basename-of-repo-root>`
- **AND** the key is the same on every machine for the same directory name

### Requirement: Branch slug normalization

The branch slug SHALL be `git rev-parse --abbrev-ref HEAD` with `/` replaced by `--`; outside a repo the slug SHALL be `none`.

#### Scenario: Feature branch

- **WHEN** HEAD is `feature/login-fix`
- **THEN** the branch slug is `feature--login-fix`

### Requirement: Worktree slot

The worktree slot SHALL be the basename of the git common dir, falling back to the basename of the working directory outside a repo. In practice the common dir basename is `.git` and is **shared by every worktree of the same repo** — the slot does not distinguish linked worktrees from the main checkout (the worktree scope's value is machine/checkout-local setup, not per-worktree identity).

#### Scenario: Linked worktree shares the slot

- **WHEN** a linked worktree and the main checkout of the same repo both resolve the worktree scope
- **THEN** both get the same slot (basename of the shared common dir, typically `.git`)
- **AND** their worktree env resolves to the same path

#### Scenario: Bare repo

- **WHEN** the checkout is a bare repo whose common dir is the bare dir itself
- **THEN** the slot is the bare dir's basename

### Requirement: Scope resolution is exact and total

Each scope SHALL resolve to exactly one path under the effective knowledge root: `global` → `<workspace-root>/rules.md`; `repo` → `repos/<repo-key>/env.md`; `branch` → `repos/<repo-key>/branches/<branch-slug>/notes.md`; `worktree` → `repos/<repo-key>/worktrees/<slot>/env.md`; `group` → `<group-root>/repos/<repo-key>/env.md`. Resolution SHALL never guess or search.

#### Scenario: Branch scope resolves deterministically

- **WHEN** `turu resolve branch` runs in a repo with key `github.com/org/repo` on branch `main`
- **THEN** the target is `<knowledge-root>/repos/github.com/org/repo/branches/main/notes.md`

#### Scenario: Group scope without an active group

- **WHEN** `turu resolve group` runs and no group is configured for this repo
- **THEN** the command fails with an error and a next-step suggestion
- **AND** no path is returned

#### Scenario: Unknown scope rejected

- **WHEN** `turu resolve nonsense` runs
- **THEN** the command fails with an error listing the valid scopes
