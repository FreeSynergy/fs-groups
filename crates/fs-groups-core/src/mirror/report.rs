//! What a synchronisation did.
//!
//! Pattern: Value object — plain lists of group ids, one per outcome.

use fs_store::ObjectId;

/// Result of `Mirror::sync_to`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Groups that were not in the directory and are now.
    pub created: Vec<ObjectId>,
    /// Groups that differed (also a manual change in the directory) and were brought back.
    pub updated: Vec<ObjectId>,
    /// Groups that already matched; nothing was written.
    pub unchanged: Vec<ObjectId>,
    /// Groups we mirrored before that no longer exist here, now removed from the directory.
    pub removed: Vec<ObjectId>,
    /// Groups the directory refused, with its message. The next run tries again.
    pub failed: Vec<(ObjectId, String)>,
    /// Edges written as a flat list, where the directory gives the child's people more than
    /// `may_do` (provisional, see `right_flow`). Reported, not adapted.
    pub divergences: Vec<Divergence>,
    /// `(parent, child)`: a child the directory does not hold (refused, or not applied), so the
    /// parent was applied without it. The next run tries again.
    pub unnested: Vec<(ObjectId, ObjectId)>,
    /// Groups applied except for these people, whom the directory could not map to an account.
    pub unmapped: Vec<(ObjectId, Vec<String>)>,
    /// Stored objects that cannot be read (a group, a membership, a delegation, or a group whose
    /// type was changed). Where they touch a group nobody is taken out of it (what is wanted may be
    /// short); revocations elsewhere go on. The run is not quiet.
    pub unreadable: Vec<ObjectId>,
    /// Groups on a loop (bad data from a second writer): their edges are written flat, never nested.
    pub looped: Vec<ObjectId>,
    /// `(group, people)` left out of a group that carries an administrative right (`Manage`, or a
    /// member with an admin role) because they are agents or not known to be human (ADR 0056).
    pub left_out: Vec<(ObjectId, Vec<String>)>,
    /// `(group, people)` in a group that may be administrative (an unreadable object could not be
    /// placed) whom the host could not vouch for. They stay; known agents are out.
    pub unchecked: Vec<(ObjectId, Vec<String>)>,
    /// The ledger could not be tidied up after the run (the intent written before the run stands, so
    /// nothing is lost; ids of removed groups stay in it until the next run).
    pub ledger_error: Option<String>,
}

/// An edge where the directory gives the people of `group` what `parent` holds, although `may_do`
/// does not (provisional, see `right_flow`): they are listed in `parent`, flat or through a nested
/// group. With the safe reading that is **every** edge, the nested ones too. Belonging is kept
/// (nobody loses what `effective` says); the rights are the part that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The child group.
    pub group: ObjectId,
    /// The parent it is nested in.
    pub parent: ObjectId,
}

impl SyncReport {
    /// Whether nothing needed doing and nothing failed.
    pub fn is_quiet(&self) -> bool {
        self.created.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.failed.is_empty()
            && self.unnested.is_empty()
            && self.unreadable.is_empty()
            && self.looped.is_empty()
            && self.unmapped.is_empty()
            && self.ledger_error.is_none()
    }
}
