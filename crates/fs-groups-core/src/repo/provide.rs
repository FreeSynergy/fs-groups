//! `GroupRepo` as a provider of the traits.
//!
//! Pattern: Adapter — each trait method calls the inherent method of the same concern (inherent
//! methods win in paths like `GroupRepo::get`), so callers with and without the trait agree.

use std::sync::Arc;

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_store::{ObjectId, Timestamp};

use super::handle::GroupRepo;
use crate::error::Result;
use crate::model::{
    Cursor, Delegation, Delivery, EffectiveMember, Group, Invitation, Member, Membership, Offer,
    Page, ParentRequest, Right, RightFlow, Source,
};
use crate::traits::{
    Delegations, Follows, GroupFeed, GroupListener, Groups, Invitations, Memberships, Subscription,
};

#[async_trait]
impl Groups for GroupRepo {
    fn capabilities(&self) -> Capabilities {
        Self::support()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Group>> {
        GroupRepo::get(self, id).await
    }
    async fn list(&self, cursor: Option<&Cursor>) -> Result<Page<Group>> {
        GroupRepo::list(self, cursor).await
    }
    async fn parents(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        GroupRepo::parents(self, id).await
    }
    async fn children(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        GroupRepo::children(self, id).await
    }
    async fn create(&self, name: &str, kind: &str) -> Result<Group> {
        GroupRepo::create(self, name, kind).await
    }
    async fn rename(&self, id: ObjectId, name: &str) -> Result<Group> {
        GroupRepo::rename(self, id, name).await
    }
    async fn set_kind(&self, id: ObjectId, kind: &str) -> Result<Group> {
        GroupRepo::set_kind(self, id, kind).await
    }
    async fn set_right_flow(&self, id: ObjectId, flow: RightFlow) -> Result<Group> {
        GroupRepo::set_right_flow(self, id, flow).await
    }
    async fn delete(&self, id: ObjectId) -> Result<()> {
        GroupRepo::delete(self, id).await
    }
    async fn request_parent(&self, child: ObjectId, parent: ObjectId) -> Result<ParentRequest> {
        GroupRepo::request_parent(self, child, parent).await
    }
    async fn approve_parent(&self, request: ObjectId) -> Result<Group> {
        GroupRepo::approve_parent(self, request).await
    }
    async fn decline_parent(&self, request: ObjectId) -> Result<()> {
        GroupRepo::decline_parent(self, request).await
    }
    async fn parent_requests(&self, parent: ObjectId) -> Result<Vec<ParentRequest>> {
        GroupRepo::parent_requests(self, parent).await
    }
    async fn remove_parent(&self, id: ObjectId, parent: ObjectId) -> Result<Group> {
        GroupRepo::remove_parent(self, id, parent).await
    }
    async fn ancestors(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        GroupRepo::ancestors(self, id).await
    }
    async fn descendants(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        GroupRepo::descendants(self, id).await
    }
}

#[async_trait]
impl Memberships for GroupRepo {
    async fn direct(&self, group: ObjectId) -> Result<Vec<Membership>> {
        GroupRepo::direct(self, group).await
    }
    async fn add(&self, group: ObjectId, member: Member, role: &str) -> Result<Membership> {
        GroupRepo::add(self, group, member, role).await
    }
    async fn remove(&self, membership: ObjectId) -> Result<()> {
        GroupRepo::remove(self, membership).await
    }
    async fn effective(&self, group: ObjectId) -> Result<Vec<EffectiveMember>> {
        GroupRepo::effective(self, group).await
    }
    async fn groups_of(&self, person: &str) -> Result<Vec<ObjectId>> {
        GroupRepo::groups_of(self, person).await
    }
    async fn is_member(&self, group: ObjectId, person: &str) -> Result<bool> {
        GroupRepo::is_member(self, group, person).await
    }
}

#[async_trait]
impl Invitations for GroupRepo {
    async fn list(&self, group: ObjectId) -> Result<Vec<Invitation>> {
        self.invitations(group).await
    }
    async fn invite(
        &self,
        group: ObjectId,
        invitee: &str,
        role: &str,
        expires: Option<Timestamp>,
    ) -> Result<Invitation> {
        GroupRepo::invite(self, group, invitee, role, expires).await
    }
    async fn accept(&self, id: ObjectId, person: &str, now: Timestamp) -> Result<Membership> {
        GroupRepo::accept(self, id, person, now).await
    }
    async fn accept_proved(
        &self,
        id: ObjectId,
        person: &str,
        proved_address: &str,
        now: Timestamp,
    ) -> Result<Membership> {
        GroupRepo::accept_proved(self, id, person, proved_address, now).await
    }
    async fn decline(&self, id: ObjectId) -> Result<()> {
        GroupRepo::decline(self, id).await
    }
    async fn revoke(&self, id: ObjectId) -> Result<()> {
        GroupRepo::revoke(self, id).await
    }
    async fn expire_due(&self, now: Timestamp) -> Result<usize> {
        GroupRepo::expire_due(self, now).await
    }
}

#[async_trait]
impl Delegations for GroupRepo {
    async fn list(&self, group: ObjectId) -> Result<Vec<Delegation>> {
        self.delegations(group).await
    }
    async fn grant(&self, group: ObjectId, holder: Member, right: Right) -> Result<Delegation> {
        GroupRepo::grant(self, group, holder, right).await
    }
    async fn revoke(&self, id: ObjectId) -> Result<()> {
        self.revoke_right(id).await
    }
    async fn may_do(&self, group: ObjectId, person: &str, right: Right) -> Result<bool> {
        GroupRepo::may_do(self, group, person, right).await
    }
}

impl GroupFeed for GroupRepo {
    fn subscribe(&self, listener: Arc<dyn GroupListener>) -> Subscription {
        self.feed.add(listener)
    }
}

#[async_trait]
impl Follows for GroupRepo {
    async fn deliver(
        &self,
        group: ObjectId,
        recipient: &str,
        rights: &[Right],
        pass_on: bool,
    ) -> Result<(Delivery, Offer)> {
        GroupRepo::deliver(self, group, recipient, rights, pass_on).await
    }
    async fn pass_on(
        &self,
        source: ObjectId,
        recipient: &str,
        rights: &[Right],
    ) -> Result<(Delivery, Offer)> {
        GroupRepo::pass_on(self, source, recipient, rights).await
    }
    async fn revoke_delivery(&self, delivery: ObjectId) -> Result<()> {
        GroupRepo::revoke_delivery(self, delivery).await
    }
    async fn deliveries(&self, group: ObjectId) -> Result<Vec<Delivery>> {
        GroupRepo::deliveries(self, group).await
    }
    async fn receive(&self, sender: &str, offer: &Offer) -> Result<Source> {
        GroupRepo::receive(self, sender, offer).await
    }
    async fn withdraw(&self, sender: &str, source: &str, delivery: ObjectId) -> Result<Source> {
        GroupRepo::withdraw(self, sender, source, delivery).await
    }
    async fn sources(&self, group: ObjectId) -> Result<Vec<Source>> {
        GroupRepo::sources(self, group).await
    }
    async fn remove_source(&self, id: ObjectId) -> Result<()> {
        GroupRepo::remove_source(self, id).await
    }
}
