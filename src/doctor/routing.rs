//! Doctor's routing-mode check (gh-3): the declared mode and effective
//! repo/branch route; stale checkout-layer content warns under
//! machine-local routing.

use genesis::doctor::CheckEntry;

use super::ROUTING;
use super::checks::with_fix;
use crate::config::{Resolved, Routing};
use crate::workspace::Facts;

const ROUTING_DESC: &str = "routing mode is declared and honored";

/// Doctor's routing-mode check (gh-3): the declared mode and effective
/// repo/branch route; a stale checkout layer warns under machine-local.
pub(super) fn routing_check(facts: &Facts, resolved: &Resolved) -> CheckEntry {
    let store_route = machine_store_route(facts, resolved);
    match resolved.routing {
        Routing::RepoLocal => CheckEntry::pass(
            ROUTING,
            ROUTING_DESC,
            "repo/branch → <checkout>/.whisper/ (decision A)",
        ),
        Routing::MachineLocal => {
            let stale = stale_checkout_layer(facts);
            if stale.is_empty() {
                CheckEntry::pass(ROUTING, ROUTING_DESC, store_route)
            } else {
                with_fix(
                    CheckEntry::warn(
                        ROUTING,
                        ROUTING_DESC,
                        format!(
                            "machine-local routing is active but the checkout layer still carries content: [{}] — recall composes it, but new writes go to the store",
                            stale.join(", ")
                        ),
                    ),
                    "migrate the content (turu bundle pack repo → unpack outside the checkout) or delete the checkout slot files",
                )
            }
        }
    }
}

/// Checkout-layer slot files that still carry content under machine-local
/// routing — leftovers from the previous mode that would silently diverge.
fn stale_checkout_layer(facts: &Facts) -> Vec<String> {
    facts
        .repo_local_root
        .iter()
        .flat_map(|root| {
            [
                root.join("env.md"),
                root.join("branches")
                    .join(&facts.branch_slug)
                    .join("notes.md"),
            ]
        })
        .filter(|p| {
            std::fs::read_to_string(p)
                .map(|c| !c.trim().is_empty())
                .unwrap_or(false)
        })
        .map(|p| format!("`{}`", p.display()))
        .collect()
}

/// Human label for the machine-local store destination of this checkout.
fn machine_store_route(facts: &Facts, resolved: &Resolved) -> String {
    format!(
        "repo/branch → machine-local store ({})",
        resolved
            .knowledge_root()
            .join("repos")
            .join(&facts.repo_key)
            .display()
    )
}
