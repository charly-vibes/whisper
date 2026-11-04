//! Shared dispatch helpers: scope naming, write-destination resolution,
//! scope keys, path maps, and text collection.

use std::io::IsTerminal;

use whisper::{WhisperError, config, workspace};

pub(crate) fn scope_name(scope: workspace::Scope) -> &'static str {
    match scope {
        workspace::Scope::Global => "global",
        workspace::Scope::Repo => "repo",
        workspace::Scope::Branch => "branch",
        workspace::Scope::Worktree => "worktree",
        workspace::Scope::Group => "group",
    }
}

/// Destination for write-path verbs (resolve/append): the checkout's
/// repo-local root for `repo`/`branch` (decision A — knowledge travels with
/// the repo's git), unless `--global` selects the machine-local store.
/// `--global` on other scopes is an error (they have a single home).
pub(crate) fn append_target(
    scope: workspace::Scope,
    global: bool,
    private: bool,
    facts: &workspace::Facts,
    resolved: &config::Resolved,
) -> whisper::Result<workspace::Target> {
    if global && private {
        return Err(WhisperError::new(
            "--global and --private are mutually exclusive — one destination per write",
        )
        .with_suggestion(
            "pick the machine-local store (--global) or the checkout's private zone (--private)",
        ));
    }
    if global {
        return global_target(scope, facts, resolved);
    }
    if private {
        return workspace::private_zone_target(scope, facts, resolved);
    }
    workspace::resolve(scope, facts, resolved)
}

/// The `--global` escape-hatch destination: the machine-local store for
/// `repo`/`branch`; an error on other scopes (they have a single home).
fn global_target(
    scope: workspace::Scope,
    facts: &workspace::Facts,
    resolved: &config::Resolved,
) -> whisper::Result<workspace::Target> {
    if !matches!(scope, workspace::Scope::Repo | workspace::Scope::Branch) {
        return Err(
            WhisperError::new("--global applies only to the repo and branch scopes")
                .with_suggestion(format!(
                    "{} always writes to its single home — drop --global",
                    scope_name(scope)
                )),
        );
    }
    if facts.repo_local_root.is_none() {
        return Err(WhisperError::new(
            "--global is redundant outside a git repo — repo/branch already resolve to the store",
        )
        .with_suggestion("drop --global"));
    }
    let checkout = workspace::Facts {
        repo_local_root: None,
        ..facts.clone()
    };
    workspace::resolve(scope, &checkout, resolved)
}

/// Scope key for an appended entry, matching where it landed: append
/// derives it from the resolved target so store and checkout never share
/// an id space (decision A — same text in both layers stays distinct).
pub(crate) fn append_scope_key(
    scope: workspace::Scope,
    target: &workspace::Target,
    facts: &workspace::Facts,
    resolved: &config::Resolved,
) -> String {
    // Layer variants of the repo/branch layout, each its own id space:
    // committed checkout (@local), private zone (@private), and the
    // machine-local store (bare scope key).
    let layer = |sub: &str| -> Option<std::path::PathBuf> {
        match (&scope, &facts.repo_local_root) {
            (workspace::Scope::Repo, Some(root)) => Some(root.join(sub).join("env.md")),
            (workspace::Scope::Branch, Some(root)) => Some(
                root.join(sub)
                    .join("branches")
                    .join(&facts.branch_slug)
                    .join("notes.md"),
            ),
            _ => None,
        }
    };
    let suffix = if layer("").as_deref() == Some(target.path.as_path()) {
        "@local"
    } else if layer("private").as_deref() == Some(target.path.as_path()) {
        "@private"
    } else {
        ""
    };
    format!("{}{}", workspace::scope_key(scope, facts, resolved), suffix)
}

pub(crate) fn paths_for(
    facts: &workspace::Facts,
    resolved: &config::Resolved,
) -> whisper::Result<serde_json::Map<String, serde_json::Value>> {
    Ok([
        workspace::Scope::Global,
        workspace::Scope::Repo,
        workspace::Scope::Branch,
        workspace::Scope::Worktree,
        workspace::Scope::Group,
    ]
    .into_iter()
    .map(|scope| {
        // Group scope may legitimately be inactive; record it as null.
        let value = match workspace::resolve(scope, facts, resolved) {
            Ok(t) => serde_json::json!(t.path),
            Err(_) => serde_json::Value::Null,
        };
        (scope_name(scope).to_string(), value)
    })
    .collect())
}

/// Payload for append, in order of explicitness: --text (repeatable) and the
/// positional text win over stdin; when neither carries text and stdin is not
/// a TTY (piped/redirected), stdin is read automatically so the natural
/// `turu append <scope> <<'EOF'` heredoc form works without --stdin.
pub(crate) fn collect_text(
    text: Option<&str>,
    texts: &[String],
    use_stdin: bool,
) -> whisper::Result<String> {
    if use_stdin {
        return read_stdin();
    }
    let mut joined = texts.to_vec();
    if let Some(t) = text {
        joined.push(t.to_owned());
    }
    if !joined.is_empty() {
        return Ok(joined.join("\n"));
    }
    if !std::io::stdin().is_terminal() {
        return read_stdin();
    }
    Ok(String::new())
}

fn read_stdin() -> whisper::Result<String> {
    std::io::read_to_string(std::io::stdin()).map_err(WhisperError::from)
}
