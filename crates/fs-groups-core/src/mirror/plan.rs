//! Comparing the wanted state with the directory's.
//!
//! Pattern: Specification (pure) — no I/O, so the decision "create, update, leave, remove" is
//! testable alone. Removal is only ever planned for ids we know we mirrored.

use std::collections::{BTreeMap, BTreeSet};

use fs_store::ObjectId;

use crate::traits::DirectoryGroup;

/// One step toward the wanted state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Not in the directory: apply.
    Create(DirectoryGroup),
    /// Different in the directory: apply.
    Update(DirectoryGroup),
    /// Equal: leave.
    Keep(ObjectId),
    /// Mirrored before, not wanted now, still there: remove.
    Remove(ObjectId),
}

/// The steps to bring `current` (what the directory holds) to `desired`, in the order of `desired`
/// (children first), removals last. `mirrored` are the ids we put there earlier; a directory group
/// outside `desired` and `mirrored` is not ours and is left alone.
pub fn plan(
    desired: &[DirectoryGroup],
    current: &[DirectoryGroup],
    mirrored: &BTreeSet<ObjectId>,
) -> Vec<Change> {
    let there: BTreeMap<_, _> = current
        .iter()
        .map(|g| (g.id, g.clone().normalised()))
        .collect();
    let wanted: BTreeSet<_> = desired.iter().map(|g| g.id).collect();
    let mut steps: Vec<_> = desired
        .iter()
        .map(|g| {
            let g = g.clone().normalised();
            match there.get(&g.id) {
                None => Change::Create(g),
                Some(c) if *c == g => Change::Keep(g.id),
                Some(_) => Change::Update(g),
            }
        })
        .collect();
    steps.extend(
        mirrored
            .iter()
            .filter(|id| !wanted.contains(id) && there.contains_key(id))
            .map(|id| Change::Remove(*id)),
    );
    steps
}
