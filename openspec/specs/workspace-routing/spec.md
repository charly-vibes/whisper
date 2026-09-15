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

The worktree slot SHALL be the basename of the git common dir (distinguishing linked worktrees), falling back to the basename of the working directory outside a repo.

#### Scenario: Linked worktree

- **WHEN** the checkout is a linked worktree whose common dir basename is `.git`
- **THEN** the worktree slot is the basename of that common dir

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
