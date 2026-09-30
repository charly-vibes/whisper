# Configuration

Where knowledge gets written is layered. Precedence: **repo > group > global**.

## Global

`~/.config/whisper/config.toml`:

```toml
workspace_root = "~/.whisper"   # default

[groups.cv-tools]
root = "~/para/areas/dev/gh/charly/knowledge"
repos = ["cv/charly-vibes/whisper"]   # explicit membership (optional)
```

A **group** routes knowledge from a set of repos into one shared workspace
directory instead of each repo's private slot.

## Repo (private)

`<repo>/.whisper/config.toml` (gitignored — never committed):

```toml
group = "cv-tools"              # join a group defined in the global config
workspace_root = "/path/to/dir" # or fully override the workspace root
```

A repo can join a group two ways: privately, via its own `.whisper/config.toml`,
or centrally, by listing its canonical key in the group's `repos` array.

> **Note:** a repo-private `workspace_root` overrides *everything* — including
> the global scope, so the repo's `rules.md` resolves inside the private root
> instead of the shared workspace. `turu doctor` reports this shadowing and
> warns when the two `rules.md` files diverge. A relative `workspace_root` is
> anchored to the directory containing the repo config (not the process cwd),
> so calls from any subdirectory resolve to the same root.

## Knowledge transport & the private zone

| Scope | In a checkout (default) | `--global` escape hatch |
|---|---|---|
| `repo` | `<repo>/.whisper/env.md` (committed) | `<workspace>/repos/<key>/env.md` |
| `branch` | `<repo>/.whisper/branches/<slug>/notes.md` (committed) | `<workspace>/repos/<key>/branches/<slug>/notes.md` |
| `--private` repo/branch | `<repo>/.whisper/private/…` — **gitignored, never pushed** | — |

Transport decision: the repo's own git carries the shared knowledge. Repo-local
publication assumes a pushable checkout — `--global` is the fallback for
read-only checkouts (CI, no-push contributors).

The private zone is enforced by `turu doctor` with three checks:

- `turu.private-ignore` — the zone is *effectively* ignored (warning + fix hint otherwise)
- `turu.private-tracked` — **fails prominently** if any file under `private/` is tracked
- `turu.private-leaks` — advisory lint for machine paths, hostnames, and token-shaped strings in public repo-local files (surfaces via `warnings[]`)

Git history is forever — prevention only. No turu verb can retract committed
knowledge; that is exactly why the private zone exists and why the tracked-files
check fails instead of warning.
