//! Delegation: a finer right than "all or nothing".
//!
//! Pattern: Value objects + Specification (`Right::covers` is the one rule: `manage` implies all).

use fs_objects::ObjectId;

use super::Member;

/// What a holder may do with a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Right {
    /// See the group and its members.
    Read,
    /// Add members.
    AddMembers,
    /// Remove members.
    RemoveMembers,
    /// Everything, including the other three.
    Manage,
}

impl Right {
    /// Whether holding `self` allows `asked`: the same right, or `Manage` for any.
    pub fn covers(self, asked: Right) -> bool {
        self == asked || self == Right::Manage
    }

    /// Stable text used in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Right::Read => "read",
            Right::AddMembers => "addMembers",
            Right::RemoveMembers => "removeMembers",
            Right::Manage => "manage",
        }
    }

    /// Inverse of [`Right::as_str`].
    pub fn parse(s: &str) -> Option<Right> {
        [
            Right::Read,
            Right::AddMembers,
            Right::RemoveMembers,
            Right::Manage,
        ]
        .into_iter()
        .find(|r| r.as_str() == s)
    }
}

/// One right held by a person or a group on a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delegation {
    /// Id of the delegation.
    pub id: ObjectId,
    /// The group the right is about.
    pub group: ObjectId,
    /// Who holds it: a person, or a group (then its direct members; see `right_flow::recipients`).
    pub holder: Member,
    /// The right.
    pub right: Right,
}
