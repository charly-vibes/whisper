//! Workspace unit tests (facts, keys, scope parsing, layout).

use super::*;
use crate::config::{Resolved, Routing};

use super::uw::Uw;

#[test]
fn canonical_key_variants() {
    assert_eq!(
        canonical_key("git@cv:charly-vibes/whisper.git"),
        "cv/charly-vibes/whisper"
    );
    assert_eq!(
        canonical_key("https://github.com/u/r.git"),
        "github.com/u/r"
    );
    assert_eq!(canonical_key("http://github.com/u/r"), "github.com/u/r");
    assert_eq!(
        canonical_key("ssh://git@github.com/u/r.git"),
        "github.com/u/r"
    );
    assert_eq!(canonical_key("git@github.com:u/r.git"), "github.com/u/r");
}

#[test]
fn scope_parse() {
    use std::str::FromStr;
    assert_eq!(Scope::from_str("global").uw(), Scope::Global);
    assert_eq!(Scope::from_str("Worktree").uw(), Scope::Worktree);
    assert!(Scope::from_str("galaxy").is_err());
}

#[test]
fn resolve_branch_in_a_checkout_is_repo_local() {
    let facts = Facts {
        repo_key: "cv/charly-vibes/whisper".into(),
        branch_slug: "feature--x".into(),
        worktree_slot: "whisper".into(),
        repo_local_root: Some(PathBuf::from("/checkout/.whisper")),
    };
    let resolved = Resolved {
        workspace_root: PathBuf::from("/tmp/ws"),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    let t = resolve(Scope::Branch, &facts, &resolved).uw();
    assert_eq!(
        t.path,
        PathBuf::from("/checkout/.whisper/branches/feature--x/notes.md")
    );
    let r = resolve(Scope::Repo, &facts, &resolved).uw();
    assert_eq!(r.path, PathBuf::from("/checkout/.whisper/env.md"));
}

#[test]
fn resolve_repo_branch_without_a_checkout_falls_back_to_store() {
    let facts = Facts {
        repo_key: "cv/charly-vibes/whisper".into(),
        branch_slug: "feature--x".into(),
        worktree_slot: "whisper".into(),
        repo_local_root: None,
    };
    let resolved = Resolved {
        workspace_root: PathBuf::from("/tmp/ws"),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    let t = resolve(Scope::Branch, &facts, &resolved).uw();
    assert_eq!(
        t.path,
        PathBuf::from("/tmp/ws/repos/cv/charly-vibes/whisper/branches/feature--x/notes.md")
    );
    let r = resolve(Scope::Repo, &facts, &resolved).uw();
    assert_eq!(
        r.path,
        PathBuf::from("/tmp/ws/repos/cv/charly-vibes/whisper/env.md")
    );
}

#[test]
fn group_scope_without_group_is_error() {
    let facts = Facts {
        repo_key: "github.com/u/r".into(),
        branch_slug: "main".into(),
        worktree_slot: "r".into(),
        repo_local_root: None,
    };
    let resolved = Resolved {
        workspace_root: PathBuf::from("/tmp/ws"),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    let err = resolve(Scope::Group, &facts, &resolved).unwrap_err();
    assert!(err.suggestion.is_some());
}

#[test]
fn legacy_variants_flags_bare_and_matching_colon_forms_only() {
    let tmp = tempfile::tempdir().uw();
    for name in ["whisper", "host:whisper", "ak:akielbowicz", "github.com"] {
        std::fs::create_dir_all(tmp.path().join("repos").join(name)).uw();
    }
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };

    let variants = legacy_variants(&uw::ws_facts("github.com/u/whisper"), &resolved);

    assert_eq!(
        variants,
        vec!["host:whisper".to_string(), "whisper".to_string()]
    );
}

#[test]
fn consolidate_refuses_when_the_canonical_dir_lies_under_the_private_zone() {
    // whisper-4xo (2.6): regression guard — if routing ever relocates
    // the knowledge root into the checkout's private zone, consolidate
    // must refuse rather than move or merge anything under it.
    let tmp = tempfile::tempdir().uw();
    let facts = Facts {
        repo_key: "github.com/u/whisper".into(),
        branch_slug: "main".into(),
        worktree_slot: ".git".into(),
        repo_local_root: Some(tmp.path().join(".whisper")),
    };
    let resolved = Resolved {
        workspace_root: tmp.path().join(".whisper/private"),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };
    // A legacy variant with content, so the pre-guard code would act.
    let variant = tmp.path().join(".whisper/private/repos/whisper/env.md");
    std::fs::create_dir_all(variant.parent().uw()).uw();
    std::fs::write(&variant, "legacy line\n").uw();

    let err = consolidate(&facts, &resolved).unwrap_err();
    assert!(err.to_string().contains("private zone"), "{err}");
    // Nothing moved, merged, or removed under the zone.
    assert_eq!(std::fs::read_to_string(&variant).uw(), "legacy line\n");
}

#[test]
fn private_zone_is_the_exact_top_level_path_only() {
    // whisper-4xo: a `private/` name nested deeper or with siblings
    // does NOT inherit the guarantee.
    let facts = uw::checkout_path_facts();
    let inside = [
        "/checkout/.whisper/private/env.md",
        "/checkout/.whisper/private/branches/main/notes.md",
    ];
    let outside = [
        "/checkout/.whisper/privateX/env.md",
        "/checkout/.whisper/branches/main/private/notes.md",
        "/checkout/.whisper/env.md",
    ];
    for p in inside {
        assert!(is_in_private_zone(&PathBuf::from(p), &facts), "{p}");
    }
    for p in outside {
        assert!(!is_in_private_zone(&PathBuf::from(p), &facts), "{p}");
    }
    // Outside a checkout there is no zone at all.
    let detached = Facts {
        repo_local_root: None,
        ..facts
    };
    assert!(!is_in_private_zone(
        &PathBuf::from("/checkout/.whisper/private/env.md"),
        &detached
    ));
}

#[test]
fn private_zone_state_is_fail_closed_when_git_fails() {
    // whisper-tlp decision note: genesis::git::is_ignored distinguishes
    // NotIgnored from Error. A git failure (e.g. check-ignore run outside
    // a repository, exit >= 128) cannot prove the zone safe, so it maps
    // to Exposed at THIS call site — the repair path must run.
    let not_a_repo = tempfile::tempdir().uw();
    let facts = Facts {
        repo_key: "github.com/u/r".into(),
        branch_slug: "main".into(),
        worktree_slot: "r".into(),
        repo_local_root: Some(not_a_repo.path().join(".whisper")),
    };
    assert_eq!(
        private_zone_state(&facts),
        PrivateZoneState::Exposed,
        "git failure must fail closed, not read as safe"
    );
}

#[test]
fn consolidate_moves_whole_dir_when_canonical_missing() {
    let tmp = tempfile::tempdir().uw();
    let repos = tmp.path().join("repos");
    let legacy = repos.join("whisper");
    std::fs::create_dir_all(legacy.join("branches/main")).uw();
    std::fs::write(legacy.join("env.md"), "legacy fact\n").uw();
    std::fs::write(legacy.join("branches/main/notes.md"), "note\n").uw();
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };

    let report = consolidate(&uw::ws_facts("github.com/u/whisper"), &resolved).uw();

    assert_eq!(report.variants, vec!["whisper".to_string()]);
    let canon = repos.join("github.com/u/whisper");
    assert_eq!(
        std::fs::read_to_string(canon.join("env.md")).uw(),
        "legacy fact\n"
    );
    assert_eq!(
        std::fs::read_to_string(canon.join("branches/main/notes.md")).uw(),
        "note\n"
    );
    assert!(!legacy.exists());
}

#[test]
fn consolidate_merges_extending_never_duplicating() {
    let tmp = tempfile::tempdir().uw();
    let repos = tmp.path().join("repos");
    let canon = repos.join("github.com/u/whisper");
    let legacy = repos.join("whisper");
    std::fs::create_dir_all(&canon).uw();
    std::fs::create_dir_all(&legacy).uw();
    std::fs::write(canon.join("env.md"), "shared fact\ncanonical only\n").uw();
    std::fs::write(legacy.join("env.md"), "legacy only\nshared fact\n\n").uw();
    let resolved = Resolved {
        workspace_root: tmp.path().to_path_buf(),
        group: None,
        shadowed_global_root: None,
        routing: Routing::RepoLocal,
    };

    let report = consolidate(&uw::ws_facts("github.com/u/whisper"), &resolved).uw();

    let merged = std::fs::read_to_string(canon.join("env.md")).uw();
    assert_eq!(merged, "shared fact\ncanonical only\nlegacy only\n");
    assert_eq!(report.merged, vec![canon.join("env.md")]);
    assert!(!legacy.exists());
}

/// Facts with a repo-local root at `/checkout/.whisper`.
fn checkout_path_facts() -> Facts {
    Facts {
        repo_key: "github.com/u/r".into(),
        branch_slug: "main".into(),
        worktree_slot: ".git".into(),
        repo_local_root: Some(PathBuf::from("/checkout/.whisper")),
    }
}
