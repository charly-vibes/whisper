//! First-party managed skill packs (dont's `skill_pack` pattern).
//!
//! The whisper skill ships from this repo so the skill and the binary
//! evolve together. `turu skill install [<dir>]` writes the pack;
//! `turu doctor` flags stale packs via content hashes.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

pub type PackFiles = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackState {
    Pass,
    Stale,
    Missing,
}

impl PackState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Stale => "stale",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackHealth {
    pub name: String,
    pub state: PackState,
    pub detail: String,
}

/// Generate all files for a named first-party managed skill pack.
pub fn generate_pack(pack_name: &str) -> Result<PackFiles, String> {
    match pack_name {
        "whisper" => Ok(generate_whisper()),
        other => Err(format!("unknown managed skill pack: {other:?}")),
    }
}

/// SHA-256 of all files sorted by relative path, concatenated (path + "\0" + content).
pub fn pack_content_hash(files: &PackFiles) -> String {
    let mut hasher = Sha256::new();
    for (path, content) in files {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(content.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// SHA-256 of on-disk files under `dir`, same algorithm as `pack_content_hash`.
pub fn disk_content_hash(dir: &Path) -> io::Result<String> {
    let files = read_dir_files(dir)?;
    Ok(pack_content_hash(&files))
}

fn read_dir_files(dir: &Path) -> io::Result<PackFiles> {
    let mut result = BTreeMap::new();
    collect_files(dir, dir, &mut result)?;
    Ok(result)
}

fn collect_files(base: &Path, current: &Path, out: &mut PackFiles) -> io::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(base, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(base)
                .map_err(io::Error::other)?
                .to_string_lossy()
                .to_string();
            let content = std::fs::read_to_string(&path)?;
            out.insert(rel, content);
        }
    }
    Ok(())
}

const MANAGED: &str =
    "<!-- MANAGED BY turu — DO NOT EDIT MANUALLY. Regenerate: turu skill install -->\n\n";

fn generate_whisper() -> PackFiles {
    let mut files = BTreeMap::new();
    files.insert("whisper.md".to_string(), router_skill());
    files.insert("subs/key.md".to_string(), sub(
        "whisper/key",
        "Derive canonical repo key, branch slug, and worktree slot",
        "1. `turu key --json` — deterministic; same repo → same key on every machine\n2. Never derive these paths by hand while the binary is present",
    ));
    files.insert("subs/resolve.md".to_string(), sub(
        "whisper/resolve",
        "Get the exact write destination for a knowledge scope",
        "1. `turu resolve global|repo|branch|worktree|group --json`\n2. If a `<!-- TURU:START -->` block exists in AGENTS.md, read the routing map from there\n3. Group scope routes to the shared group workspace when one is active",
    ));
    files.insert("subs/append.md".to_string(), sub(
        "whisper/append",
        "Extend-don't-duplicate knowledge into the right file",
        "1. Search first — check the target file for an existing note on the topic\n2. `turu append <scope> --text \"...\"` — appends verbatim, creates parents\n3. Extend or correct existing notes instead of adding duplicates; no secrets, ever",
    ));
    files.insert("subs/init.md".to_string(), sub(
        "whisper/init",
        "Create the workspace layout for this checkout",
        "1. `turu init --json` — creates rules.md, repo slot, branch slot, worktree slot\n2. Never overwrites existing files\n3. `turu sync` afterwards to (re)install the AGENTS.md managed block",
    ));
    files.insert("subs/status.md".to_string(), sub(
        "whisper/status",
        "One envelope with every relevant path and existence flag",
        "1. `turu status --json` — workspace root, group, repo key, slugs, paths\n2. `turu check --json` — legacy key variants, missing files, undefined groups",
    ));
    files.insert("subs/sync.md".to_string(), sub(
        "whisper/sync",
        "Regenerate the managed routing block in AGENTS.md",
        "1. `turu sync --json` — updates `<!-- TURU:START -->` in AGENTS.md in place\n2. Agents in this repo read paths from the block instead of re-deriving them",
    ));
    files.insert("subs/doctor.md".to_string(), sub(
        "whisper/doctor",
        "Deep workspace diagnostics",
        "1. `turu doctor --json` — layout, group health, legacy keys, managed block, skill pack staleness\n2. Each failing check carries a `fix` hint; apply it",
    ));
    files.insert("subs/link-manual.md".to_string(), sub_fallback(
        "whisper/link-manual",
        "Link a plan to the workspace (manual fallback — no turu command yet)",
        "Follow the manual procedure in incitaciones `content/distilled/whisper/references/` if present; otherwise: derive the repo key with `turu key`, then record the beads epic ID in the branch slot's context.md via `turu append branch`.",
    ));
    files.insert("subs/decommission-manual.md".to_string(), sub_fallback(
        "whisper/decommission-manual",
        "Decommission a branch/worktree slot (manual fallback)",
        "1. Confirm the branch is merged/decommissioned in beads if available\n2. Remove the slot directory under the branch/worktree root (canonical key from `turu key`)\n3. Keep env.md knowledge by promoting it: repo-wide facts → `turu append repo`, global → `turu append global`",
    ));
    files.insert("subs/consolidate-manual.md".to_string(), sub_fallback(
        "whisper/consolidate-manual",
        "Migrate legacy repo-key directories to the canonical key (manual fallback)",
        "1. `turu doctor --json` lists variants under `turu.legacy-keys`\n2. Move directory contents into the canonical key dir, merge env.md by extending, never duplicating\n3. Re-run `turu doctor` — the check must pass",
    ));
    files
}

fn sub(name: &str, description: &str, protocol: &str) -> String {
    format!(
        "{MANAGED}---\nname: {name}\ndescription: \"{description}\"\n---\n\n# {name}\n\n{description}.\n\n## Protocol\n\n{protocol}\n"
    )
}

fn sub_fallback(name: &str, description: &str, protocol: &str) -> String {
    sub(name, description, protocol)
}

fn router_skill() -> String {
    format!(
        "{MANAGED}---\nname: whisper\ndescription: \"Deterministic knowledge workspace: init, check, status, link, decommission, knowledge routing. Delegate to the turu CLI; manual fallbacks when absent. Trigger on '/w', '/whisper', 'init workspace', 'check workspace', 'workspace status', 'link plan', 'decommission'.\"\ntools: Read, Write, Edit, Bash\n---\n\n# Whisper — Deterministic Operational Knowledge\n\nManage the whisper knowledge workspace. The `turu` CLI owns all mechanical\nsteps: canonical repo keys, branch slugs, worktree slots, scope routing.\nNever hand-roll those paths while the binary is present.\n\n## Decision procedure\n\n1. Is `turu` available? (`turu key --json` succeeds)\n2. If **yes** — everything mechanical is a turu command:\n\n| Intent | Command |\n|---|---|\n| Derive repo key / branch slug / worktree slot | `turu key --json` |\n| Find the write destination for a scope | `turu resolve <scope> --json` |\n| Record knowledge (extend, don't duplicate) | `turu append <scope> --text \"...\"` |\n| Create the workspace layout | `turu init --json` |\n| Inspect paths and existence | `turu status --json` |\n| Validate / detect legacy keys | `turu check --json` · `turu doctor --json` |\n| Refresh the AGENTS.md routing block | `turu sync --json` |\n\n   Scopes: `global` (rules.md), `repo` (env.md), `branch` (notes.md),\n   `worktree` (env.md), `group` (shared workspace). If a\n   `<!-- TURU:START -->` block exists in AGENTS.md, read the routing map\n   from there instead of running resolve.\n\n3. If **no** — use the manual fallbacks in `subs/*-manual.md` and the\n   incitaciones references; suggest installing via `cargo install whisper-vibes`.\n\n## Invariants (with or without the binary)\n\n- **No secrets anywhere.** Never write tokens, keys, passwords, or PII.\n- **Extend, don't duplicate.** Append to or correct existing notes.\n- **One repo, one key.** Canonical key is a pure function of the remote URL.\n- **Search first** before creating a new entry.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generate_whisper_is_deterministic() {
        let a = generate_pack("whisper").unwrap();
        let b = generate_pack("whisper").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn pack_contains_router_and_all_subs() {
        let files = generate_pack("whisper").unwrap();
        assert_eq!(files.len(), 11);
        let router = &files["whisper.md"];
        for expected in [
            "name: whisper",
            "turu key --json",
            "turu resolve <scope>",
            "turu append <scope>",
            "subs/*-manual.md",
            "No secrets anywhere",
        ] {
            assert!(
                router.contains(expected),
                "router must contain {expected:?}"
            );
        }
        for sub in files.keys() {
            assert!(
                sub.starts_with("whisper.md") || sub.starts_with("subs/"),
                "{sub}"
            );
            assert!(files[sub].starts_with("<!-- MANAGED BY turu"), "{sub}");
        }
    }

    #[test]
    fn disk_hash_matches_pack_hash() {
        let dir = TempDir::new().unwrap();
        let files = generate_pack("whisper").unwrap();
        for (rel, content) in &files {
            let dest = dir.path().join(rel);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(&dest, content).unwrap();
        }
        assert_eq!(
            pack_content_hash(&files),
            disk_content_hash(dir.path()).unwrap()
        );
    }

    #[test]
    fn unknown_pack_is_error() {
        assert!(generate_pack("nope").is_err());
    }
}
