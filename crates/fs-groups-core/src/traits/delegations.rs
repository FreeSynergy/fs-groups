//! The [`Delegations`] contract: finer rights than "all or nothing".
//!
//! Pattern: Repository (interface) + Null Object. `may_do` only answers the question; no other
//! method enforces it yet (see `DESIGN.md`).

use async_trait::async_trait;
use fs_store::ObjectId;

use crate::error::Result;
use crate::model::{Delegation, Member, Right};
use crate::unsupported::not_possible;

/// Who may do what with a group. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Delegations: Send + Sync {
    /// `right.read`: the delegations of a group.
    async fn list(&self, group: ObjectId) -> Result<Vec<Delegation>>;

    /// `right.grant`: give a person or a group a right.
    async fn grant(&self, _group: ObjectId, _holder: Member, _right: Right) -> Result<Delegation> {
        not_possible("right.grant")
    }
    /// `right.grant`: take a delegation back.
    async fn revoke(&self, _id: ObjectId) -> Result<()> {
        not_possible("right.grant")
    }
    /// `right.read`: whether `person` holds `right` on `group`, directly or through a group they
    /// belong to. `Manage` implies the others.
    async fn may_do(&self, _group: ObjectId, _person: &str, _right: Right) -> Result<bool> {
        not_possible("right.read")
    }
}
