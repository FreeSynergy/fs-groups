//! What a group's [`RightFlow`] setting means. **Provisional** (ADR 0055 point 6): the exact meaning of
//! up and down is still being settled with the owner, so every rule about it lives in this file.
//!
//! Pattern: Strategy (pure) — [`reach`] is the one function that decides where a right goes; the
//! repository only asks it. [`directory_expresses`] is the one decision about what a directory can hold.
//! The group the right would pass FROM decides: a walk starts at the group holding the right, takes the
//! direction of that group's setting, and goes on from a group only if that group passes the same way.
//! Each group is visited once, so a walk ends on any data, cyclic bad data too.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fs_store::ObjectId;

use crate::graph::Graph;
use crate::model::{Group, RightFlow};

/// The settings of all groups, by id.
pub fn settings(groups: &[Group]) -> BTreeMap<ObjectId, RightFlow> {
    groups.iter().map(|g| (g.id, g.right_flow)).collect()
}

/// The groups a right on `from` counts on, `from` included, nearest first.
/// `None`: `from` only. `Down`: also its children, and theirs while each passes down. `Up`: the same
/// toward parents. A group not in `settings` passes nothing.
pub fn reach(
    graph: &Graph,
    settings: &BTreeMap<ObjectId, RightFlow>,
    from: ObjectId,
) -> Vec<ObjectId> {
    let flow_of = |id: ObjectId| settings.get(&id).copied().unwrap_or_default();
    let direction = flow_of(from);
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    let mut out = vec![from];
    while let Some(cur) = queue.pop_front() {
        if flow_of(cur) != direction {
            continue;
        }
        let next = match direction {
            RightFlow::None => &[][..],
            RightFlow::Down => graph.children(cur),
            RightFlow::Up => graph.parents(cur),
        };
        for n in next.iter().filter(|n| seen.insert(**n)) {
            out.push(*n);
            queue.push_back(*n);
        }
    }
    out
}

/// The groups on which a right placed on one of them counts on `target`: every group whose [`reach`]
/// contains `target` (`target` itself always).
pub fn sources(
    graph: &Graph,
    settings: &BTreeMap<ObjectId, RightFlow>,
    target: ObjectId,
) -> Vec<ObjectId> {
    settings
        .keys()
        .copied()
        .filter(|g| reach(graph, settings, *g).contains(&target))
        .collect()
}

/// Whether a directory like Kanidm holds the nested edge `child` in a parent as the child's setting
/// says. There, a member of a child is always `memberof` the parent, which is only what a child that
/// passes up says; the directory cannot be told otherwise per group.
pub fn directory_expresses(child: RightFlow) -> bool {
    child == RightFlow::Up
}
