//! What a group's [`RightFlow`] setting means. **Provisional** (ADR 0055 point 6): the exact meaning of
//! up and down, and who consents to a link, is still being answered by the owner. This is the **safe
//! reading**: nothing is given away unless every group involved agrees. Every rule about it lives in
//! this file, in two clauses that can each be changed alone:
//!
//! * **Effect** ([`reach`], [`sources`]): on which groups a right placed on a group applies.
//! * **Recipients** ([`recipients`]): the people of which groups receive a right held by a group.
//!   [`directory_expresses`] is its edge-level form for a directory.
//!
//! Pattern: Strategy (pure) — the repository only asks these functions. [`directory_expresses`] is the
//! one decision about what a directory can hold. Each group is visited once, so a walk ends on any
//! data, cyclic bad data too.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fs_store::ObjectId;

use crate::graph::Graph;
use crate::model::{Group, RightFlow};

/// The settings of all groups, by id.
pub fn settings(groups: &[Group]) -> BTreeMap<ObjectId, RightFlow> {
    groups.iter().map(|g| (g.id, g.right_flow)).collect()
}

// ---- Effect clause -------------------------------------------------------------------------------

/// The groups a right placed on `from` applies to, `from` included, nearest first; empty if `from` is
/// not a known group.
///
/// `None`: `from` only. `Down` (`Up`): also the children (parents) whose own setting is `Down`
/// (`Up`) too, and onward through such groups. **Both ends must agree**: a group that does not pass
/// the same way is not reached, so a child or parent can never have a right pushed onto it, nor
/// pull one from a group that set `None`.
pub fn reach(
    graph: &Graph,
    settings: &BTreeMap<ObjectId, RightFlow>,
    from: ObjectId,
) -> Vec<ObjectId> {
    let Some(direction) = settings.get(&from).copied() else {
        return vec![];
    };
    let agrees = |id: &ObjectId| settings.get(id) == Some(&direction);
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    let mut out = vec![from];
    while let Some(cur) = queue.pop_front() {
        let next = match direction {
            RightFlow::None => &[][..],
            RightFlow::Down => graph.children(cur),
            RightFlow::Up => graph.parents(cur),
        };
        for n in next.iter().filter(|n| agrees(n) && seen.insert(**n)) {
            out.push(*n);
            queue.push_back(*n);
        }
    }
    out
}

/// The groups on which a right placed on one of them applies to `target`: every group whose
/// [`reach`] contains `target` (`target` itself always, if known). Computed by walking back from
/// `target`, so it costs one walk, not one per group.
pub fn sources(
    graph: &Graph,
    settings: &BTreeMap<ObjectId, RightFlow>,
    target: ObjectId,
) -> BTreeSet<ObjectId> {
    let Some(&own) = settings.get(&target) else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::from([target]);
    // A right comes from a group that passes `own` too, along a chain of groups that all do.
    let back = |id: ObjectId| match own {
        RightFlow::None => &[][..],
        RightFlow::Down => graph.parents(id),
        RightFlow::Up => graph.children(id),
    };
    let mut queue = VecDeque::from([target]);
    while let Some(cur) = queue.pop_front() {
        for n in back(cur) {
            if settings.get(n) == Some(&own) && out.insert(*n) {
                queue.push_back(*n);
            }
        }
    }
    out
}

// ---- Recipient clause ----------------------------------------------------------------------------

/// The groups whose **direct** members receive a right held by group `holder`, `holder` first;
/// empty for an unknown holder.
///
/// A holder that does not pass `Up` has itself only: a board that passes down does not reach an
/// intern in a child, a new parent does not capture the rights of a holder. A holder that passes
/// `Up` is joined by the children that pass `Up` too, and onward through such children: their
/// members rise into the holder, as in a directory like Kanidm, where the members of a child get
/// what the parent holds. **Both ends must agree**, as in [`reach`]. Provisional: the owner may
/// still split this into two settings.
pub fn recipients(
    graph: &Graph,
    settings: &BTreeMap<ObjectId, RightFlow>,
    holder: ObjectId,
) -> Vec<ObjectId> {
    let Some(&own) = settings.get(&holder) else {
        return vec![];
    };
    let mut out = vec![holder];
    if own != RightFlow::Up {
        return out;
    }
    let mut seen = BTreeSet::from([holder]);
    let mut queue = VecDeque::from([holder]);
    while let Some(cur) = queue.pop_front() {
        for c in graph.children(cur) {
            if settings.get(c) == Some(&RightFlow::Up) && seen.insert(*c) {
                out.push(*c);
                queue.push_back(*c);
            }
        }
    }
    out
}

// ---- Directory -----------------------------------------------------------------------------------

/// Whether the nested edge `child` in `parent` may go to a directory like Kanidm as it is. There,
/// every member of a child gets what the parent holds, always and only upward. The edge is faithful
/// when [`recipients`] says the same: the child is among the parent's recipients, which for a direct
/// edge means both pass `Up`. Anywhere else the mirror writes a flat list instead (the people of the
/// child are listed in the parent), see `repo/mirror.rs`.
pub fn directory_expresses(
    settings: &BTreeMap<ObjectId, RightFlow>,
    child: ObjectId,
    parent: ObjectId,
) -> bool {
    settings.get(&child) == Some(&RightFlow::Up) && settings.get(&parent) == Some(&RightFlow::Up)
}
