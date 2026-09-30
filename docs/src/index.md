# What is whisper

`whisper` (the `turu` binary) makes the [incitaciones](https://github.com/charly-vibes/incitaciones) `whisper` skill deterministic.

The whisper skill asks an agent to manage `~/.whisper/` — canonical repo keys,
branch slugs, worktree slots, and scope-based knowledge routing — by hand-rolling
shell pipelines. Every invocation is a fresh chance for the model to mis-derive
a path. This tool turns that mechanical layer into a binary: the skill prompts
call the CLI, and the same inputs always produce the same outputs.

**Why:** agents accumulate operational knowledge across sessions, but "where does
this note go?" is a question models answer unreliably. Canonical key derivation
is a pure function of the remote URL — the same repo always yields the same key
on every machine — so routing stops depending on prompt bookkeeping.

**Install:**

```bash
cargo install whisper-vibes
```

The command is `turu` — following the song's *turututu* riff. `turututu` and
`whisper` run as aliases, so existing scripts and skill prompts keep working.

Part of the [charly-vibes](https://github.com/charly-vibes) tool suite; built on
[genesis-vibes](https://github.com/charly-vibes/genesis) for the JSON envelope,
config, and CLI conventions shared across the family.
