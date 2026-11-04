//! Layout tests: legacy-key consolidation and multi-layer init.

use crate::config::{Resolved, Routing};

use super::uw::Uw;
use super::*;

#[test]
fn consolidate_merges_nested_files_into_existing_dirs() {
    let (_tmp, canon, legacy, resolved) = nested_merge_fixture();
    consolidate(&uw::ws_facts("github.com/u/whisper"), &resolved).uw();
    t_merge_asserts(&canon, &legacy);
}

/// Nested canonical + legacy dirs with overlapping and novel notes.
type Fix = (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    Resolved,
);

fn nested_merge_fixture() -> Fix {
    let tmp = tempfile::tempdir().uw();
    let repos = tmp.path().join("repos");
    let canon = repos.join("github.com/u/whisper");
    let legacy = repos.join("whisper");
    std::fs::create_dir_all(canon.join("branches/main")).uw();
    std::fs::create_dir_all(legacy.join("branches/main")).uw();
    std::fs::create_dir_all(legacy.join("branches/feature--x")).uw();
    std::fs::write(canon.join("branches/main/notes.md"), "shared note\n").uw();
    std::fs::write(
        legacy.join("branches/main/notes.md"),
        "shared note\nlegacy note\n",
    )
    .uw();
    std::fs::write(
        legacy.join("branches/feature--x/notes.md"),
        "feature note\n",
    )
    .uw();
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    (tmp, canon, legacy, resolved)
}

/// Post-merge expectations: extended note, new branch note, legacy gone.
fn t_merge_asserts(canon: &std::path::Path, legacy: &std::path::Path) {
    assert_eq!(
        std::fs::read_to_string(canon.join("branches/main/notes.md")).uw(),
        "shared note\nlegacy note\n"
    );
    assert_eq!(
        std::fs::read_to_string(canon.join("branches/feature--x/notes.md")).uw(),
        "feature note\n"
    );
    assert!(!legacy.exists());
}

#[test]
fn consolidate_noop_without_variants() {
    let tmp = tempfile::tempdir().uw();
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };

    let report = consolidate(&uw::ws_facts("github.com/u/r"), &resolved).uw();

    assert!(report.variants.is_empty());
    assert!(report.moved.is_empty());
    assert!(report.merged.is_empty());
    assert!(!tmp.path().join("repos").exists());
}

#[test]
fn append_creates_then_extends() {
    let tmp = tempfile::tempdir().uw();
    let checkout = tempfile::tempdir().uw();
    let facts = Facts {
        repo_key: "github.com/u/r".into(),
        branch_slug: "main".into(),
        worktree_slot: "r".into(),
        repo_local_root: Some(checkout.path().join(".whisper")),
    };
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    let t = resolve(Scope::Repo, &facts, &resolved).uw();
    // Decision A: repo knowledge lands in the checkout, not the store.
    assert_eq!(t.path, checkout.path().join(".whisper/env.md"));
    assert!(t.ensure().uw());
    t.append("first fact").uw();
    t.append("second fact\n\n").uw();
    let content = std::fs::read_to_string(&t.path).uw();
    assert_eq!(content, "first fact\nsecond fact\n");
    assert!(!t.ensure().uw());
}

#[test]
fn init_provisions_both_layers_when_in_a_checkout() {
    let (tmp, checkout, facts, resolved) = uw::checkout_fixture();
    init(&facts, &resolved).uw();
    assert_layers_provisioned(&tmp, &checkout);

    // Outside a checkout only the store layer exists — the scope files
    // are already there (no re-creation); only the branch-slot extras
    // (context.md/plan.md) are new, since the first init's branch slot
    // lived in the checkout layer.
    let bare = Facts {
        repo_local_root: None,
        ..facts
    };
    let report = init(&bare, &resolved).uw();
    assert_only_branch_extras(&report);
}

/// Both layers carry the scope files after init in a checkout.
fn assert_layers_provisioned(tmp: &tempfile::TempDir, checkout: &tempfile::TempDir) {
    assert!(checkout.path().join(".whisper/env.md").exists());
    assert!(
        checkout
            .path()
            .join(".whisper/branches/main/notes.md")
            .exists()
    );
    assert!(tmp.path().join("repos/github.com/u/r/env.md").exists());
    assert!(
        tmp.path()
            .join("repos/github.com/u/r/branches/main/notes.md")
            .exists()
    );
}

/// A bare (store-only) init may only add context.md/plan.md extras.
fn assert_only_branch_extras(report: &InitReport) {
    for p in &report.created {
        let name = p.file_name().uw().to_string_lossy();
        assert!(
            name == "context.md" || name == "plan.md",
            "unexpected creation: {p:?}"
        );
    }
}

/// Create the canonical/legacy branch dirs and seed their notes.
fn seed_nested_merge(canon: &std::path::Path, legacy: &std::path::Path) {
    std::fs::create_dir_all(canon.join("branches/main")).uw();
    std::fs::create_dir_all(legacy.join("branches/main")).uw();
    std::fs::create_dir_all(legacy.join("branches/feature--x")).uw();
    std::fs::write(canon.join("branches/main/notes.md"), "shared note\n").uw();
    std::fs::write(
        legacy.join("branches/main/notes.md"),
        "shared note\nlegacy note\n",
    )
    .uw();
    std::fs::write(
        legacy.join("branches/feature--x/notes.md"),
        "feature note\n",
    )
    .uw();
}
