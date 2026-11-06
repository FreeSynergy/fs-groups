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
    /// Nested edges the directory holds although the child's setting says otherwise; it cannot
    /// express them. Reported, not adapted: what is applied stays as it is.
    pub divergences: Vec<Divergence>,
}

/// A group whose right-flow setting the directory cannot express (provisional, see `right_flow`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The child group whose setting is not kept.
    pub group: ObjectId,
    /// The parent in the directory it is nested in, against its setting.
    pub parent: ObjectId,
}

impl SyncReport {
    /// Whether nothing needed doing and nothing failed.
    pub fn is_quiet(&self) -> bool {
        self.created.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.failed.is_empty()
    }
}
