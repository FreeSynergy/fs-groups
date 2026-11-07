//! The [`Invitations`] contract: ask someone to join, and their answer.
//!
//! Pattern: Repository (interface) + Null Object; the life cycle is the State pattern
//! (`invitation_state.rs`). The time is always passed in, so nothing here needs a clock.

use async_trait::async_trait;
use fs_objects::{ObjectId, Timestamp};

use crate::error::Result;
use crate::model::{Invitation, Membership};
use crate::unsupported::not_possible;

/// Invitations to groups. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Invitations: Send + Sync {
    /// `invite.read`: the invitations of a group, in any state.
    async fn list(&self, group: ObjectId) -> Result<Vec<Invitation>>;

    /// `invite.create`: invite someone; `expires` = `None` never runs out.
    async fn invite(
        &self,
        _group: ObjectId,
        _invitee: &str,
        _role: &str,
        _expires: Option<Timestamp>,
    ) -> Result<Invitation> {
        not_possible("invite.create")
    }
    /// `invite.answer`: `person` accepts at `now`; this creates the membership. Only the invitee can
    /// accept: `person` must be the invitee exactly, else `Error::NotInvitee`.
    async fn accept(&self, _id: ObjectId, _person: &str, _now: Timestamp) -> Result<Membership> {
        not_possible("invite.answer")
    }
    /// `invite.answer`: accept an invitation sent to an e-mail address. **Provisional.** The caller
    /// says which verified address `person` proved; it is compared with the invitee ignoring case.
    /// Nothing is verified here.
    async fn accept_proved(
        &self,
        _id: ObjectId,
        _person: &str,
        _proved_address: &str,
        _now: Timestamp,
    ) -> Result<Membership> {
        not_possible("invite.answer")
    }
    /// `invite.answer`: the invitee declines.
    async fn decline(&self, _id: ObjectId) -> Result<()> {
        not_possible("invite.answer")
    }
    /// `invite.create`: the group withdraws a pending invitation.
    async fn revoke(&self, _id: ObjectId) -> Result<()> {
        not_possible("invite.create")
    }
    /// `invite.answer`: mark every pending invitation that ran out by `now` as expired; how many.
    async fn expire_due(&self, _now: Timestamp) -> Result<usize> {
        not_possible("invite.answer")
    }
}
