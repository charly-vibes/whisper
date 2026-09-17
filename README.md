> *"Time can never mend*
> *The careless whispers of a good friend*
> *To the heart and mind, ignorance is kind*
> *There's no comfort in the truth, pain is all you'll find"*
> — George Michael, *Careless Whisper*

# whisper

`whisper` is a Rust CLI that makes the [incitaciones](https://github.com/charly-vibes/incitaciones) `whisper` skill deterministic.

The whisper skill asks an agent to manage `~/.whisper/` — canonical repo keys, branch slugs, worktree slots, and scope-based knowledge routing — by hand-rolling shell pipelines. Every invocation is a fresh chance for the model to mis-derive a path. This tool turns that mechanical layer into a binary: the skill prompts call the CLI, and the same inputs always produce the same outputs.

**The command is `turu`** — following the song's *turututu* riff. `turututu` and `whisper` run as aliases, so existing scripts and skill prompts keep working.

Part of the [charly-vibes](https://github.com/charly-vibes) tool suite; built on [genesis-vibes](https://github.com/charly-vibes/genesis) for the JSON envelope, config, and CLI conventions shared across the family.

## What it does

- `turu key` — canonical repo key, branch slug, and worktree slot for the current checkout (the pure determinism core)
- `turu resolve <scope>` — the exact destination path for a knowledge scope: `global`, `repo`, `branch`, `worktree`, or `group`
- `turu append <scope> --text "..." [--topic k] [--supersedes id]` — structured, idempotent entries (sha2 ids over scope + second-precision UTC + text) written into the right file; freeform content preserved verbatim
- `turu recall <scope> [--topic k] [--budget bytes] [--include-superseded]` — ranked, budget-bounded slice serving (`all` composes scopes in precedence order); the envelope reports the context-horizon boundary (`served_bytes`, `budget_unused`, `entries_skipped`)
- `turu distill <scope> --begin|--commit --revision id` — two-phase contract: turu snapshots into immutable revisions and guards the commit against concurrent drift; the calling agent does the semantic rewrite into the working file
- `turu bundle pack|unpack` — deterministic, transport-agnostic knowledge bundles keyed by repo key; unpack merges extend-without-duplicate and never overwrites
- `turu init` — create the workspace layout
- `turu status` — one envelope with every relevant path and existence flag
- `turu check` — detect legacy key variants, undefined groups, missing files
- `turu doctor` — deep diagnostics: layout integrity, group health, legacy keys, managed block, skill pack staleness
- `turu sync` — inject the deterministic routing map into `AGENTS.md` as a `<!-- TURU:START -->` managed block, so agents read paths from a file instead of re-deriving them
- `turu skill install` — ship the whisper skill from this repo: router + sub-skills (including manual fallbacks), content-hashed so `turu doctor` flags staleness. Targets `~/.claude/skills` or any dir; incitaciones keeps a thin pointer to it

## Install

```bash
cargo install whisper-vibes
```

Or from source:

```bash
git clone git@cv:charly-vibes/whisper.git && cd whisper && just install
```

## Configuration

Where knowledge gets written is layered. Precedence: **repo > group > global**.

**Global** — `~/.config/whisper/config.toml`:

```toml
workspace_root = "~/.whisper"   # default

[groups.cv-tools]
root = "~/para/areas/dev/gh/charly/knowledge"
repos = ["cv/charly-vibes/whisper"]   # explicit membership (optional)
```

A **group** routes knowledge from a set of repos into one shared workspace directory instead of each repo's private slot.

**Repo (private)** — `<repo>/.whisper/config.toml` (gitignored — never committed):

```toml
group = "cv-tools"              # join a group defined in the global config
workspace_root = "/path/to/dir" # or fully override the workspace root
```

A repo can join a group two ways: privately, via its own `.whisper/config.toml`, or centrally, by listing its canonical key in the group's `repos` array.

> **Note:** a repo-private `workspace_root` overrides *everything* — including the global scope, so the repo's `rules.md` resolves inside the private root instead of the shared workspace. `turu doctor` reports this shadowing and warns when the two `rules.md` files diverge. A relative `workspace_root` is anchored to the directory containing the repo config (not the process cwd), so calls from any subdirectory resolve to the same root.

## Skill integration

The distilled whisper skill in incitantes replaces its shell-snippet procedures with:

```
turu key        # derive repo key / branch slug / worktree slot
turu resolve branch
turu append repo --text "deploy requires vault login"
turu recall repo --topic deploy --budget 400   # slice, newest first, boundary reported
```

Agents in any repo with a `TURU` managed block read the routing map straight from `AGENTS.md` — no prompt bookkeeping required. `turu sync` refreshes the block; `turu doctor` flags it when it drifts.

Determinism guarantee: canonical key derivation is a pure function of the remote URL; the same repo always yields the same key on every machine.

## Roadmap

- `turu consolidate` — migrate legacy repo-key directories into the canonical key (detection already ships in `turu doctor`)

## License

MIT
