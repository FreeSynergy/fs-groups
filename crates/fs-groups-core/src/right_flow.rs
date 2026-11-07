//! What a group's [`RightFlow`] setting means. **Provisional** (ADR 0055 point 6): the exact meaning of
//! up and down, and who consents to a link, is still being answered by the owner. This is the **safe
//! reading**: nothing is given away unless every group involved agrees. Every rule about it lives in
//! this file, in two clauses that can each be changed alone:
//!
//! * **Effect** ([`reach`], [`sources`]): on which groups a right placed on a group applies.
//! * **Recipients** ([`recipients`]): the people of which groups receive a right held by a group.
//!   [`directory_expresses`] and [`directory_nests`] are the two statements about a directory.
//!
//! Pattern: Strategy (pure) — the repository only asks these functions. [`directory_expresses`] is the
//! one decision about what a directory can hold. Each group is visited once, so a walk ends on any
//! data, cyclic bad data too.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fs_objects::ObjectId;

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

/// The groups whose **direct** members receive a right held by group `holder`: the holder itself,
/// whatever anybody set; empty for an unknown holder.
///
/// This is the safe reading and it stands until the owner answers: a board that passes down does not
/// reach an intern in a child, an Up holder does not reach the members of an Up child (they were
/// never asked), and a new parent does not capture the rights of a holder. A person who should
/// receive a right is made a member of the holder or is granted the right. Widening this (two
/// settings, consent on both sides) is **open with the owner**; it would change this function only.
pub fn recipients(settings: &BTreeMap<ObjectId, RightFlow>, holder: ObjectId) -> Vec<ObjectId> {
    if settings.contains_key(&holder) {
        vec![holder]
    } else {
        vec![]
    }
}

// ---- Directory -----------------------------------------------------------------------------------

/// Whether a directory like Kanidm holds the nested edge `child` in `parent` without giving more than
/// `may_do`. There every member of a child gets what the parent holds; here they get it only if the
/// child is among the parent's [`recipients`], which for a child of the parent never happens. So
/// **every nested edge is a divergence** and the report lists them all, the nested ones too.
pub fn directory_expresses(
    settings: &BTreeMap<ObjectId, RightFlow>,
    child: ObjectId,
    parent: ObjectId,
) -> bool {
    recipients(settings, parent).contains(&child)
}

/// Whether the mirror writes the edge `child` in `parent` as a nested group (option B, ADR 0056,
/// provisional): only where the rights are meant to pass on, which is when both pass `Up`, the way a
/// directory like Kanidm passes them from a child upward. Any other edge is written flat. This is a
/// statement about the directory only; it does not make `may_do` follow it, so a nested edge still
/// gives the child's people more in the directory than `may_do` (reported as a divergence).
pub fn directory_nests(
    settings: &BTreeMap<ObjectId, RightFlow>,
    child: ObjectId,
    parent: ObjectId,
) -> bool {
    settings.get(&child) == Some(&RightFlow::Up) && settings.get(&parent) == Some(&RightFlow::Up)
}
