# Getting started

## Install

```bash
cargo install whisper-vibes
```

Or from source:

```bash
git clone git@cv:charly-vibes/whisper.git && cd whisper && just install
```

## Create the workspace

```bash
turu init        # create the ~/.whisper layout (never overwrites)
```

## Daily loop

Derive where you are, write what you learned, recall what you need:

```bash
turu key                                  # repo key / branch slug / worktree slot
turu resolve branch                       # the exact destination path for a scope

turu append repo --text "deploy requires vault login" --topic deploy
turu recall repo --topic deploy --budget 400   # slice, newest first, boundary reported
```

Every read and write is one JSON envelope; `recall` reports the context-horizon
boundary (`served_bytes`, `budget_unused`, `entries_skipped`).

## Share knowledge through git

By default, `repo` and `branch` scopes write into the checkout's committed
`.whisper/` directory — the repo's own git carries the shared knowledge.
Read-only checkouts (CI, no-push contributors) use the `--global` escape hatch.
Machine-specific knowledge (local paths, hostnames, credentials) belongs in the
private zone: `turu append <scope> --private`.

## Wire it into agents

```bash
turu sync         # inject the routing map into AGENTS.md as a TURU managed block
turu doctor       # verify layout, groups, managed block, privacy contract
```

Agents in any repo with a `TURU` managed block read the routing map straight
from `AGENTS.md` — no prompt bookkeeping required.
