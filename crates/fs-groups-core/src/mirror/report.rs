//! What a synchronisation did.
//!
//! Pattern: Value object — plain lists of group ids, one per outcome.

use fs_objects::ObjectId;

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
    /// Nested edges where the directory gives more than the groups' own rights do (provisional,
    /// see `right_flow`). Reported, not adapted: what is applied stays as it is.
    pub divergences: Vec<Divergence>,
    /// `(parent, child)`: a child the directory does not hold (refused, or not applied), so the
    /// parent was applied without it. The next run tries again.
    pub unnested: Vec<(ObjectId, ObjectId)>,
    /// Groups applied except for these people, whom the directory could not map to an account.
    pub unmapped: Vec<(ObjectId, Vec<String>)>,
    /// Stored groups that cannot be read as a group. They are left in the directory, never removed.
    pub unreadable: Vec<ObjectId>,
    /// The ledger could not be tidied up after the run (the intent written before the run stands, so
    /// nothing is lost; ids of removed groups stay in it until the next run).
    pub ledger_error: Option<String>,
}

/// A nested edge where the directory gives more than the groups' rights do (provisional, see
/// `right_flow`): there, every member of `group` is a member of `parent` and gets what it holds.
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
            && self.unmapped.is_empty()
            && self.ledger_error.is_none()
    }
}
