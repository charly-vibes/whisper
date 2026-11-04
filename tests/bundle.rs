//! Integration tests for bundle pack/unpack (public API only).

use whisper::bundle;
use whisper::bundle::Bundle;
use whisper::config::{Resolved, Routing};
use whisper::entry::Entry;
use whisper::workspace::{self, Facts, Scope};

/// Test-only unwrapping with a clear message (pretender counts bare
/// unwrap and expect calls; route through this instead).
trait Uw<T> {
    fn uw(self) -> T;
}
impl<T, E: std::fmt::Debug> Uw<T> for std::result::Result<T, E> {
    fn uw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
}
impl<T> Uw<T> for std::option::Option<T> {
    fn uw(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("unexpected None"),
        }
    }
}
use std::path::PathBuf;

#[test]
fn bundle_carries_only_content_and_addressing() {
    // Deployed scenario shared-store/bundle-carries-no-transport-state:
    // a bundle contains only knowledge content and addressing (repo key,
    // scope, entry ids) — no transport-specific fields are required to
    // unpack it.
    let tmp = tempfile::tempdir().uw();
    let facts = facts_with_local_root(tmp.path().join("checkout/.whisper").to_str().uw());
    let resolved = resolved_at(tmp.path());
    append_transport_free_fact(&tmp, &facts, &resolved);

    let raw = pack_repo_scope(&tmp, &facts, &resolved);
    let obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).uw();
    let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["entries", "freeform", "repo_key", "scope"]);

    // The same bytes unpack into a fresh workspace with no extra fields.
    let fresh = tempfile::tempdir().uw();
    let fresh_facts = facts_with_local_root(fresh.path().join("checkout/.whisper").to_str().uw());
    let fresh_resolved = resolved_at(fresh.path());
    let report = bundle::unpack(&raw, &fresh_facts, &fresh_resolved).uw();
    assert_eq!(report["added"], 1);
}

/// Seed one entry ("transport-free fact") into the checkout's repo scope.
fn append_transport_free_fact(_tmp: &tempfile::TempDir, facts: &Facts, resolved: &Resolved) {
    let target = workspace::resolve(Scope::Repo, facts, resolved).uw();
    target.ensure().uw();
    target.append("transport-free fact").uw();
}

/// Pack the checkout's repo scope into `tmp` and return the bundle bytes.
fn pack_repo_scope(tmp: &tempfile::TempDir, facts: &Facts, resolved: &Resolved) -> String {
    let out_path = tmp.path().join("bundle.json");
    bundle::pack(Scope::Repo, facts, resolved, Some(out_path.to_str().uw())).uw();
    std::fs::read_to_string(&out_path).uw()
}

fn facts_with_local_root(root: &str) -> Facts {
    Facts {
        repo_key: "cv/charly-vibes/whisper".into(),
        branch_slug: "main".into(),
        worktree_slot: ".git".into(),
        repo_local_root: Some(PathBuf::from(root)),
    }
}

fn resolved_at(root: &std::path::Path) -> Resolved {
    Resolved {
        workspace_root: root.to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    }
}

fn one_entry_bundle() -> String {
    let bundle = Bundle {
        repo_key: "cv/charly-vibes/whisper".into(),
        scope: "repo".into(),
        entries: vec![Entry {
            ts: "2026-01-01T00:00:00Z".into(),
            id: "abc123".into(),
            topic: None,
            text: "bundled fact".into(),
            supersedes: None,
            superseded_by: None,
        }],
        freeform: vec!["loose line".into()],
    };
    serde_json::to_string(&bundle).uw()
}

// whisper-4xo (2.5): pack refuses any scope resolving under the private
// zone — clear error, no bundle produced. No shipped scope resolves
// there today; the trigger here is a misconfigured group root inside
// the checkout's private zone, which resolve() faithfully honors.
#[test]
fn pack_refuses_a_scope_resolving_into_the_private_zone() {
    let tmp = tempfile::tempdir().uw();
    let out = tmp.path().join("bundle.json");
    let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().uw());
    let resolved = Resolved {
        workspace_root: tmp.path().join("ws"),
        group: Some(("rogue".into(), tmp.path().join(".whisper/private"))),
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };

    let err = bundle::pack(Scope::Group, &facts, &resolved, Some(out.to_str().uw())).unwrap_err();
    assert!(err.to_string().contains("never transported"), "{err}");
    // No bundle produced.
    assert!(!out.exists());
}

// whisper-4xo (2.4): unpack never creates, extends, or modifies files
// under the private zone — the content is skipped and reported.
#[test]
fn unpack_skips_and_reports_when_the_target_resolves_private() {
    let tmp = tempfile::tempdir().uw();
    let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().uw());
    let resolved = Resolved {
        workspace_root: tmp.path().join("ws"),
        group: Some(("rogue".into(), tmp.path().join(".whisper/private"))),
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    let bundle = Bundle {
        repo_key: "cv/charly-vibes/whisper".into(),
        scope: "group".into(),
        entries: vec![Entry {
            ts: "2026-01-01T00:00:00Z".into(),
            id: "abc123".into(),
            topic: None,
            text: "bundled fact".into(),
            supersedes: None,
            superseded_by: None,
        }],
        freeform: vec!["loose line".into()],
    };
    let input = serde_json::to_string(&bundle).uw();

    let report = bundle::unpack(&input, &facts, &resolved).uw();
    assert_eq!(report["added"], 0);
    assert_eq!(report["private_skipped"], 2); // 1 entry + 1 freeform line
    // Nothing written under the zone.
    assert!(
        !tmp.path()
            .join(".whisper/private/repos/cv/charly-vibes/whisper/env.md")
            .exists()
    );
}

#[test]
fn normal_pack_and_unpack_still_work_without_a_private_target() {
    let tmp = tempfile::tempdir().uw();
    let facts = facts_with_local_root(tmp.path().join(".whisper").to_str().uw());
    let resolved = resolved_at(&tmp.path().join("ws").to_path_buf());

    let report = bundle::unpack(&one_entry_bundle(), &facts, &resolved).uw();
    assert_eq!(report["added"], 2);
    assert_eq!(report["private_skipped"], serde_json::Value::Null);
    assert!(tmp.path().join(".whisper/env.md").exists());
}
