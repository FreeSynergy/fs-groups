//! The [`Memberships`] contract: who belongs to a group.
//!
//! Pattern: Repository (interface) + Null Object. The Rust form of `Memberships` in `groups.capnp`.
//! Role ids are opaque strings; their definitions stay in fs-people (ADR 0052).

use async_trait::async_trait;
use fs_store::ObjectId;

use crate::error::Result;
use crate::model::{EffectiveMember, Member, Membership};
use crate::unsupported::not_possible;

/// Memberships, direct and through nested groups. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Memberships: Send + Sync {
    /// `member.read`: the direct memberships of a group.
    async fn direct(&self, group: ObjectId) -> Result<Vec<Membership>>;

    /// `member.add`: add a person with a role (empty = plain member).
    async fn add(&self, _group: ObjectId, _member: Member, _role: &str) -> Result<Membership> {
        not_possible("member.add")
    }
    /// `member.remove`: end a membership.
    async fn remove(&self, _membership: ObjectId) -> Result<()> {
        not_possible("member.remove")
    }
    /// `member.read`: everyone who belongs, nested groups included, each person once with a path.
    async fn effective(&self, _group: ObjectId) -> Result<Vec<EffectiveMember>> {
        not_possible("member.read")
    }
    /// `member.read`: the groups a person belongs to, through nesting too.
    async fn groups_of(&self, _person: &str) -> Result<Vec<ObjectId>> {
        not_possible("member.read")
    }
    /// `member.read`: whether a person belongs to a group, through nesting too.
    async fn is_member(&self, _group: ObjectId, _person: &str) -> Result<bool> {
        not_possible("member.read")
    }
}
