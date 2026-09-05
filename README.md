> *"Time can never mend*
> *The careless whispers of a good friend*
> *To the heart and mind, ignorance is kind*
> *There's no comfort in the truth, pain is all you'll find"*
> — George Michael, *Careless Whisper*

# whisper

`whisper` is a Rust CLI that makes the [incitaciones](https://github.com/charly-vibes/incitaciones) `whisper` skill deterministic.

The whisper skill asks an agent to manage `~/.whisper/` — canonical repo keys, branch slugs, worktree slots, and scope-based knowledge routing — by hand-rolling shell pipelines. Every invocation is a fresh chance for the model to mis-derive a path. This tool turns that mechanical layer into a binary: the skill prompts call `whisper`, and the same inputs always produce the same outputs.

Part of the [charly-vibes](https://github.com/charly-vibes) tool suite; built on [genesis-vibes](https://github.com/charly-vibes/genesis) for the JSON envelope, config, and CLI conventions shared across the family.

## What it does

- `whisper key` — canonical repo key, branch slug, and worktree slot for the current checkout (the pure determinism core)
- `whisper resolve <scope>` — the exact destination path for a knowledge scope: `global`, `repo`, `branch`, `worktree`, or `group`
- `whisper append <scope> --text "..."` — extend-don't-duplicate writes into the right file, creating parents as needed
- `whisper init` — create the workspace layout
- `whisper status` — one envelope with every relevant path and existence flag
- `whisper check` — detect legacy key variants, undefined groups, missing files

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

## Skill integration

The distilled whisper skill in incitantes replaces its shell-snippet procedures with:

```
whisper key        # derive repo key / branch slug / worktree slot
whisper resolve branch
whisper append repo --text "deploy requires vault login"
```

Determinism guarantee: canonical key derivation is a pure function of the remote URL; the same repo always yields the same key on every machine.

## Roadmap

- `whisper consolidate` — migrate legacy repo-key directories into the canonical key
- `whisper doctor` — deep workspace diagnostics via `genesis::doctor`

## License

MIT
