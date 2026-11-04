//! Deep workspace diagnostics, following the genesis doctor conventions.
//!
//! Checks are assembled directly as `CheckEntry`s (config- and git-aware,
//! so the `DoctorCheck` trait's `repo_root`-only signature doesn't fit).
//! The private-zone trio (effective ignore, tracked files, leak-shape
//! lint) is the hard guarantee behind the gitignore backstop: doctor
//! detects what `init`/`sync` self-heal and what no verb can retract.

mod checks;
mod content;
mod leaks;
mod managed;
mod routing;

pub use checks::run_checks;
pub use leaks::{leak_findings, leak_warnings};

const REPO_SLOT: &str = "turu.repo-slot";
const RULES: &str = "turu.rules-md";
const BRANCH_SLOT: &str = "turu.branch-slot";
const LEGACY_KEYS: &str = "turu.legacy-keys";
const GROUP_ROOT: &str = "turu.group-root";
const SHADOWED_GLOBAL: &str = "turu.shadowed-global";
const MANAGED_BLOCK: &str = "turu.managed-block";
const MANAGED_SKILLS: &str = "turu.managed-skills";
const ENTRY_FORMAT: &str = "turu.entry-format";
const DISTILL_PENDING: &str = "turu.distill-pending";
const USAGE_STALENESS: &str = "turu.usage-staleness";
const ROUTING: &str = "turu.routing";
const PRIVATE_IGNORE: &str = "turu.private-ignore";
const PRIVATE_TRACKED: &str = "turu.private-tracked";
const PRIVATE_LEAKS: &str = "turu.private-leaks";

#[cfg(test)]
mod tests;
