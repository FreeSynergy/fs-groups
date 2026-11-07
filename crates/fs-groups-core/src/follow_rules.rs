//! Following between independent groups (ADR 0056, "Einseitig folgen"): which pattern, and why.
//!
//! Pattern: Specification (this file, pure predicates) + State (the life of a delivery and of a source)
//! + Repository (the sources a group takes in, the deliveries it makes).
//!
//! Following is **not** nesting: there are no parents and no tree, so `graph.rs` and `nesting.rs`
//! are not touched.
//!
//! * *Repository, turned round:* the giver **delivers**, the receiver keeps a list of **sources**. A
//!   group's sources are what it has been offered, like the remotes of a version-control repository
//!   (`repo/deliveries.rs`, `repo/sources.rs`).
//! * *State:* a delivery is `Active` until the giver revokes it; a source is `Active` until the giver
//!   withdraws it (the receiver may also remove it at any time). The offer reaches the receiver as an
//!   AS2 `Invite` (proposal, the owner may change the form); it is entered without any confirmation.
//! * *Specification:* "never more than the giver has", "pass on only with the source's permission" and
//!   "the offer comes from where the transport says it comes from" are pure yes/no rules over values,
//!   in this one file, tested without a store (`tests/follow_rules.rs`).
//!
//! No other pattern of `coding.md` section 2 fits better; nothing was invented.

use fs_objects::ObjectId;

use crate::error::{Error, Result};
use crate::model::{Right, Source, SourceState};

/// The rights of an offer in a fixed order without repeats. Refused: none at all, and `Manage`,
/// which is never given to a foreign group (provisional).
pub fn normalise(rights: &[Right]) -> Result<Vec<Right>> {
    let mut out = rights.to_vec();
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(Error::Invalid("a delivery needs at least one right".into()));
    }
    if out.contains(&Right::Manage) {
        return Err(Error::Forbidden(
            "manage is never delivered to another group".into(),
        ));
    }
    Ok(out)
}

/// Whether every right in `wanted` is one that `given` allows (`Manage` would cover all).
pub fn within(given: &[Right], wanted: &[Right]) -> bool {
    wanted.iter().all(|w| given.iter().any(|g| g.covers(*w)))
}

/// Whether the receiver may pass this source on: it is active and the source allowed it.
pub fn may_pass_on(source: &Source) -> bool {
    source.state == SourceState::Active && source.offer.pass_on
}

fn host_of(iri: &str) -> Option<&str> {
    iri.strip_prefix("https://")?
        .split_once('/')
        .map(|(h, _)| h)
}

/// Whether `source_iri` lies at the origin `sender` that the transport named. Nothing is verified
/// here: this only stops an offer from claiming a source somewhere else than where it came from.
pub fn from_origin(sender: &str, source_iri: &str) -> bool {
    let sender = sender.trim();
    !sender.is_empty() && host_of(source_iri).is_some_and(|h| h.eq_ignore_ascii_case(sender))
}

/// The group id when `iri` is `https://<instance>/groups/<id>` of this instance.
pub fn local_group(instance: &str, iri: &str) -> Option<ObjectId> {
    let rest = iri.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    host.eq_ignore_ascii_case(instance)
        .then(|| path.strip_prefix("groups/")?.parse().ok())?
}
