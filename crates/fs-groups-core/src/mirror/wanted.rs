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

/// The host's verdict on a person, for the rule that an agent is never admin (ADR 0056).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Known to be a human.
    Human,
    /// Marked as an agent.
    Agent,
    /// No answer. Treated like an agent in a group known to be administrative.
    Unknown,
}

/// Which groups carry an administrative right (`Manage` on anything, or a member with an admin role).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdminRule {
    /// Groups known to be administrative, an unreadable object that touches them counted: neither an
    /// agent nor an unknown person may be in their closure.
    pub strict: BTreeSet<ObjectId>,
    /// Some unreadable object may make any group administrative and could not be placed. Then only
    /// known agents are kept out of every group; unknown people are reported, not removed.
    pub lenient_all: bool,
}

impl AdminRule {
    /// Whether `verdict` keeps a person out of the closure of `group`; `None` if the group has no rule,
    /// `Some(report_only)` otherwise.
    pub fn bars(&self, group: ObjectId, verdict: Verdict) -> Bar {
        match verdict {
            Verdict::Agent if self.strict.contains(&group) || self.lenient_all => Bar::Out,
            Verdict::Unknown if self.strict.contains(&group) => Bar::Out,
            Verdict::Unknown if self.lenient_all => Bar::Unchecked,
            _ => Bar::No,
        }
    }
}

/// What the rule says about one person in one group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    /// Allowed.
    No,
    /// Must not be in the group's closure.
    Out,
    /// May be there, but it is not known whether they should (reported).
    Unchecked,
}

/// The wanted state of a directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    /// The groups as the directory should hold them (belonging, option B), children before the
    /// groups that nest them. The rule about agents is applied later, to the final plan.
    pub groups: Vec<DirectoryGroup>,
    /// Groups on a loop; their edges are never nested.
    pub looped: Vec<ObjectId>,
    /// Stored objects that cannot be read.
    pub unreadable: Vec<ObjectId>,
    /// The groups an unreadable object touches: what is wanted for them may be short, so the sync
    /// keeps what the directory holds there.
    pub uncertain: Scope,
    /// Which groups carry an administrative right.
    pub admin: AdminRule,
}
