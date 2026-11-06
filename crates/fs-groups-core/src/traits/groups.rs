//! The [`Groups`] contract: group items and nesting.
//!
//! Pattern: Repository (interface) + Template method + Null Object — callers use `dyn Groups`. Like a
//! parent class (ADR 0045): every provider has every method; one it does not override answers an
//! explicit `Unsupported`. `ancestors` / `descendants` are provided in terms of `parents` / `children`.
//! The Rust form of `Items` in `groups.capnp`.

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

use super::walk::walk;
use crate::error::Result;
use crate::model::{Cursor, Group, Page, RightFlow};
use crate::unsupported::not_possible;

/// Groups and their nesting. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Groups: Send + Sync {
    /// What this provider can do, per capability id.
    fn capabilities(&self) -> Capabilities;
    /// `group.read`: one group, if it exists.
    async fn get(&self, id: ObjectId) -> Result<Option<Group>>;
    /// `group.read`: groups in name order, one page.
    async fn list(&self, cursor: Option<&Cursor>) -> Result<Page<Group>>;
    /// `group.read`: direct parents of a group.
    async fn parents(&self, id: ObjectId) -> Result<Vec<ObjectId>>;
    /// `group.read`: direct children of a group.
    async fn children(&self, id: ObjectId) -> Result<Vec<ObjectId>>;

    /// `group.create`: a new group with no parent.
    async fn create(&self, _name: &str, _kind: &str) -> Result<Group> {
        not_possible("group.create")
    }
    /// `group.update`: change the name.
    async fn rename(&self, _id: ObjectId, _name: &str) -> Result<Group> {
        not_possible("group.update")
    }
    /// `group.update`: change the kind.
    async fn set_kind(&self, _id: ObjectId, _kind: &str) -> Result<Group> {
        not_possible("group.update")
    }
    /// `group.update`: set whether and where the group passes its rights on.
    async fn set_right_flow(&self, _id: ObjectId, _flow: RightFlow) -> Result<Group> {
        not_possible("group.update")
    }
    /// `group.delete`: remove a group with its memberships, invitations, delegations and links.
    /// Children stay.
    async fn delete(&self, _id: ObjectId) -> Result<()> {
        not_possible("group.delete")
    }
    /// `group.nest`: make `parent` a parent of `id`. A cycle is refused.
    async fn add_parent(&self, _id: ObjectId, _parent: ObjectId) -> Result<Group> {
        not_possible("group.nest")
    }
    /// `group.nest`: end the link between `id` and `parent`.
    async fn remove_parent(&self, _id: ObjectId, _parent: ObjectId) -> Result<Group> {
        not_possible("group.nest")
    }

    /// `group.read`: all parents, grandparents, ...; each group once, nearest first.
    async fn ancestors(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        walk(self, id, true).await
    }
    /// `group.read`: all children, grandchildren, ...; each group once, nearest first.
    async fn descendants(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        walk(self, id, false).await
    }
}
