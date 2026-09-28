# design.md — add-repo-private-scope

## Context

`add-shared-bundle` shipped the deterministic bundle primitive and deliberately deferred the transport fork (beads `whisper-kt5`, design table in its design.md). This change decides it. The fork's own trigger condition was "decide when a second machine/team actually needs shared knowledge"; in the kt5 decision discussion (2026-09-28) the maintainer judged the trigger met — publication-safe travel of notes is wanted now, without waiting for a second machine to force the issue — and any future multi-machine workflow would otherwise have to trust judgment-based filtering of the global store.

Terminology, used verbatim throughout: **workspace root** = `~/.whisper` (the global store, machine-local); **repo-local root** = `<checkout>/.whisper` (committed, travels with git); **private zone** = `<checkout>/.whisper/private` (gitignored, never pushed).

## Goals / Non-Goals

- Goals:
  - Branch/repo knowledge travels with the repo (survives machine switches) without new infrastructure.
  - Privacy is a deterministic, mechanical property — a path — never an LLM/script judgment call.
  - Global store remains first-class and readable; nothing forces migration.
- Non-Goals:
  - No git-refs protocol, no remote store, no sync daemon (options B/C rejected — see Decisions).
  - No entry-level visibility metadata (rejected — see Decisions).
  - No retroactive scrubbing of git history (impossible; prevention only).
  - No migration of existing global-store entries (recall composes; migration can be a later `consolidate`-style change if ever needed).

## Decisions

- **Decision: Option A — committed repo-local `.whisper/` — with a structural private zone.**
  Transport is the repo's own git. `repo`/`branch` scopes resolve into `.whisper/` in the checkout. Privacy lives in `.whisper/private/` (gitignored), the canonical exclusion zone.
  - Alternatives considered:
    - **B (bundles under git refs, beads-style)**: no working-tree noise, but adds push/pull plumbing and a second sync protocol to maintain. Rejected for now: A achieves the actual need (notes travel) with zero new machinery; B remains available if commit noise proves painful.
    - **C (central remote store)**: violates the ownership maxim; infra to run; no team exists. Rejected.
    - **Global-store + publish-filter** (private knowledge in `~/.whisper`, filtered at publish time): rejected — publication would require deciding what is safe to sweep out of a mixed store, i.e. judgment in the trust-critical path. This repo's precedent is the opposite: *detection that warns can be loose; anything that moves or publishes data cannot*.
    - **Entry-level visibility flags** (`--visibility public` + export filter): rejected — a real feature with real surface area that quietly reintroduces the transport complexity A was chosen to avoid. Zone-level routing gets the same guarantee with near-zero machinery.

- **Decision: exclusion is by exact top-level path `.whisper/private/`, enforced inside turu, not left to git.**
  Git is the backstop; turu is the guarantee. `bundle pack` skips the subtree and refuses private-resolved scopes, `unpack` never writes into it, and `consolidate` — which today operates only on the global workspace root and structurally cannot reach the checkout — is bound by an explicit regression guard never to. No other path under `.whisper/` inherits the guarantee (a `private/` nested elsewhere is just a name).

- **Decision: `worktree` scope keeps resolving into the workspace root.**
  The worktree slot is shared by every worktree of the same repo (baseline spec), so a repo-local destination cannot denote a single checkout; and worktree knowledge is machine-local by definition, which the global store already is — no privacy boundary is lost. Routing for `global`, `group`, and `worktree` is unchanged.
  - Precedent for the shape: `.whisper/config.toml` is already gitignored inside a committed-layout directory — the private zone extends an established pattern.

- **Decision: doctor owns the hard guarantee.**
  Gitignore is not a hard guarantee: `git add -f` overrides it, and files committed *before* the ignore rule stay tracked forever. The check that makes "never pushed" true rather than intended is `git ls-files .whisper/private/` empty, surfaced by doctor as a prominent failure. The leak-shape lint on public files is advisory only (4xl precedent: warnings in the envelope, never blocking).

## Risks / Trade-offs

- **Git history is forever** → the guard is preventive (ignore rule + tracked-files check), never retroactive. A leak cleaned up later is still published; this is stated verbatim in user-facing docs.
- **Commit noise in the code repo** → knowledge commits are batched (session close, separate from code commits, like the existing beads-notes convention). Accepted; revisit option B if it actually hurts.
- **Default-destination change may surprise** → recall composes old global-store entries, so nothing becomes invisible; the managed block regenerated by `turu sync` shows the new routing explicitly.
- **Linked worktrees fork branch knowledge** → each checkout owns its own committed `.whisper/`; branch notes written in two worktrees of the same repo diverge and can conflict on merge. Accepted: the workspace root remains the shared fallback in recall's store-major composition, and option B stays the documented escape hatch if this hurts.
- **Read-only checkouts strand writes** → CI and no-push contributors writing repo/branch knowledge can never publish it. Documented fallback: `--global` escape hatch (see tasks 4.2).
- **Doctor pass-count tests shift when checks are added** → known gotcha from t6j; update the affected tests in the same red/green cycle.

## Migration Plan

1. `turu init`/`turu sync` adds the `.gitignore` rule on next run — self-healing, no manual step.
2. Existing knowledge in `~/.whisper` stays put and served (recall composes); new repo/branch writes land in the checkout.
3. Rollback: revert the resolution change; global store was never touched.

## Open Questions

- Leak-shape lint heuristics (which path/hostname/token patterns) — decide at implementation, tunable constants like `LINT_TOKENS` in 4xl.
- Does the private zone eventually want its own `resolve` exposure (`resolve branch --private`), or stay an append-time routing flag? Start with the flag; add surface only on real need.
