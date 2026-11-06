//! What the source wants a directory to hold, with everything `sync_to` needs to know about it.
//!
//! Pattern: Value object — one struct instead of five questions, so the answers belong together.

use std::collections::BTreeSet;

use fs_store::ObjectId;

use crate::traits::DirectoryGroup;

/// A set of groups, or all of them (when the data does not say which).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    /// Every group: what an unreadable object touches could not be told.
    pub all: bool,
    /// These groups.
    pub groups: BTreeSet<ObjectId>,
}

impl Scope {
    /// Whether `id` is in the scope.
    pub fn covers(&self, id: ObjectId) -> bool {
        self.all || self.groups.contains(&id)
    }

    /// Whether the scope is empty.
    pub fn is_empty(&self) -> bool {
        !self.all && self.groups.is_empty()
    }
}

/// The wanted state of a directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    /// The groups as the directory should hold them, children before the groups that nest them.
    pub groups: Vec<DirectoryGroup>,
    /// `(group, people)` left out of a group that carries an administrative right (ADR 0056).
    pub left_out: Vec<(ObjectId, Vec<String>)>,
    /// Groups on a loop; their edges are never nested.
    pub looped: Vec<ObjectId>,
    /// Stored objects that cannot be read.
    pub unreadable: Vec<ObjectId>,
    /// The groups an unreadable object touches: what is wanted for them may be short, so the sync
    /// keeps what the directory holds there (but never an agent in an administrative group).
    pub uncertain: Scope,
    /// The groups that carry an administrative right, an unreadable delegation or membership
    /// counted as carrying one (fail closed).
    pub admin: Scope,
}
