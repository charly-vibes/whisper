# add-routing-mode (whisper-rk9 / gh-3)

## Why

Setting up "all knowledge local-only" vs "all knowledge tracked in-repo" for a single repo required reverse-engineering doctor/status output — there is no persistent, declarative routing mode (GitHub issue #3):

1. `<repo>/.whisper/config.toml` supports `workspace_root` and `group`, but `repo`/`branch` scopes still resolved to the checkout's committed zone — only `global`, `worktree`, and `group` honored overrides.
2. The `--global` escape hatch is per-invocation only; no per-repo config makes it the default for repo/branch scopes.
3. Switching between "local-only" and "tracked in-repo" models required hand-editing `.gitignore`, deleting config files, and cleaning stale machine-store slots.

## What Changes

- New repo-config key `routing = "repo-local" | "machine-local"` in `<repo>/.whisper/config.toml` (default `repo-local`, the decision-A behavior).
- `resolve()` honors the mode for the repo/branch scopes: `machine-local` is the persistent equivalent of the `--global` escape hatch.
- `init` provisions the layer the declared mode resolves to — never both places for the same scope (store only under `machine-local`).
- `doctor` gains `turu.routing`: reports the effective route and warns on stale checkout-layer content under `machine-local`.
- Per-invocation flags compose on top: `--private` still routes into the checkout's private zone; group routing keeps priority when active.

## Impact

- Affected specs: `workspace-routing` (ADDED requirement), `config-precedence` (config key), `workspace-maintenance` (init provisioning + doctor check).
- Affected code: `src/config.rs` (Routing enum + parsing), `src/workspace.rs` (resolve/init), `src/doctor.rs` (new check).
