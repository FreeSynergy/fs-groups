//! The [`Follows`] contract: following between independent groups (provisional, ADR 0056).
//!
//! Pattern: Repository (interface, turned round: the giver delivers, the receiver lists sources) +
//! Null Object — every method answers `Unsupported` until a provider overrides it. Not a tree: it
//! adds no parent and no cycle rule, and it does not touch nesting.
//! The Rust form of `Follows` in `groups.capnp`.

use async_trait::async_trait;
use fs_objects::ObjectId;

use crate::error::Result;
use crate::model::{Delivery, Offer, Right, Source};
use crate::unsupported::not_possible;

/// Who allows access delivers; the other side keeps the source. Interface status: draft (ADR 0045).
///
/// Nothing here authenticates `sender`: the transport (host) supplies the origin it verified; a
/// provider only checks that the offer's source lies at that origin.
#[async_trait]
pub trait Follows: Send + Sync {
    /// `follow.deliver`: let the group `recipient` (IRI) have `rights` on `group`. Never more than the
    /// acting person holds there (`may_do`), never `Manage`; an unknown `group` is an error. Returns
    /// the delivery and the offer to send to the receiver.
    async fn deliver(
        &self,
        _group: ObjectId,
        _recipient: &str,
        _rights: &[Right],
        _pass_on: bool,
    ) -> Result<(Delivery, Offer)> {
        not_possible("follow.deliver")
    }
    /// `follow.deliver`: pass a received source on to a third group. Only if the source allowed it
    /// (`pass_on`), is active, and `rights` are within what it gave. The copy is not passable again.
    async fn pass_on(
        &self,
        _source: ObjectId,
        _recipient: &str,
        _rights: &[Right],
    ) -> Result<(Delivery, Offer)> {
        not_possible("follow.deliver")
    }
    /// `follow.deliver`: the giver ends a delivery, any time. Deliveries relayed from it are not
    /// affected here (they live on the receiver's server, which learns through `withdraw`).
    async fn revoke_delivery(&self, _delivery: ObjectId) -> Result<()> {
        not_possible("follow.deliver")
    }
    /// `follow.read`: the deliveries `group` makes, any state, oldest first.
    async fn deliveries(&self, _group: ObjectId) -> Result<Vec<Delivery>> {
        not_possible("follow.read")
    }
    /// `follow.receive`: the receiver's server enters the source **without confirmation**. The offer's
    /// recipient must be a group of this server (an unknown group is an error) and its source must lie
    /// at `sender`. Receiving the same delivery again returns the entry that exists.
    async fn receive(&self, _sender: &str, _offer: &Offer) -> Result<Source> {
        not_possible("follow.receive")
    }
    /// `follow.receive`: the giver revoked `delivery` of group `source`; the entry becomes
    /// `Withdrawn` and what was relayed from it is revoked. `sender` must be the one that sent it.
    async fn withdraw(&self, _sender: &str, _source: &str, _delivery: ObjectId) -> Result<Source> {
        not_possible("follow.receive")
    }
    /// `follow.read`: the sources `group` has entered, oldest first (this is how the receiver sees the invitation).
    async fn sources(&self, _group: ObjectId) -> Result<Vec<Source>> {
        not_possible("follow.read")
    }
    /// `follow.receive`: the receiver removes a source, any time; what was relayed from it is revoked.
    async fn remove_source(&self, _id: ObjectId) -> Result<()> {
        not_possible("follow.receive")
    }
}
