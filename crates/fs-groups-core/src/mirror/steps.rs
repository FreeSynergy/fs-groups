//! The steps of `Mirror::sync_to` that need no source: what the directory holds, how the writes are
//! ordered, how a group is applied.
//!
//! Pattern: Template method (helpers) — small functions the fixed flow in `sync.rs` calls.

use std::collections::{BTreeMap, BTreeSet};

use fs_store::ObjectId;

use super::plan::Change;
use super::report::SyncReport;
use crate::error::{Error, Result};
use crate::traits::{DirectoryGroup, GroupDirectory};

/// Whether writing `wanted` over `now` takes a person or a nested edge away.
pub(super) fn takes_away(now: &DirectoryGroup, wanted: &DirectoryGroup) -> bool {
    let wanted = wanted.clone().normalised();
    now.members.iter().any(|m| !wanted.members.contains(m))
        || now.nested.iter().any(|c| !wanted.nested.contains(c))
}

/// The groups nested below `id` in the directory, at any depth.
pub(super) fn below(
    there: &BTreeMap<ObjectId, DirectoryGroup>,
    id: ObjectId,
) -> BTreeSet<ObjectId> {
    let (mut seen, mut todo) = (BTreeSet::new(), vec![id]);
    while let Some(cur) = todo.pop() {
        for c in there
            .get(&cur)
            .map(|g| g.nested.clone())
            .unwrap_or_default()
        {
            if seen.insert(c) {
                todo.push(c);
            }
        }
    }
    seen
}

/// Apply one group with the children the directory holds, and record the outcome. Whether it was applied.
pub(super) async fn put(
    directory: &dyn GroupDirectory,
    mut group: DirectoryGroup,
    new: bool,
    present: &mut BTreeSet<ObjectId>,
    report: &mut SyncReport,
) -> bool {
    let (kept, left_out): (Vec<_>, Vec<_>) = group.nested.iter().partition(|c| present.contains(c));
    let left_out: Vec<_> = left_out.into_iter().copied().collect();
    group.nested = kept.into_iter().copied().collect();
    let applied = match directory.apply(&group).await {
        Ok(()) => true,
        Err(Error::Unmapped(people)) => {
            report.unmapped.push((group.id, people));
            true
        }
        Err(e) => {
            report.failed.push((group.id, e.to_string()));
            false
        }
    };
    if applied {
        present.insert(group.id);
        report
            .unnested
            .extend(left_out.into_iter().map(|c| (group.id, c)));
        if new {
            report.created.push(group.id);
        } else {
            report.updated.push(group.id);
        }
    }
    applied
}

/// `wanted` plus what the directory holds now (people and nested groups).
pub(crate) fn hold_still(
    mut wanted: DirectoryGroup,
    current: &BTreeMap<ObjectId, DirectoryGroup>,
) -> DirectoryGroup {
    if let Some(now) = current.get(&wanted.id) {
        wanted.members.extend(now.members.iter().cloned());
        wanted.nested.extend(now.nested.iter().copied());
    }
    wanted.normalised()
}

/// What the directory holds now (asking for a group of ours that `list` left out before deciding it
/// is gone), and the ids confirmed gone.
pub(super) async fn current_state(
    directory: &dyn GroupDirectory,
    known: &BTreeSet<ObjectId>,
    wanted: &BTreeSet<ObjectId>,
    report: &mut SyncReport,
) -> Result<(Vec<DirectoryGroup>, BTreeSet<ObjectId>)> {
    let mut current = directory.list().await?;
    let mut gone = BTreeSet::new();
    let listed: BTreeSet<_> = current.iter().map(|g| g.id).collect();
    for id in known
        .iter()
        .filter(|id| !wanted.contains(id) && !listed.contains(id))
    {
        match directory.read(*id).await {
            Ok(Some(g)) => current.push(g),
            Ok(None) => {
                gone.insert(*id);
            }
            Err(e) => report.failed.push((*id, e.to_string())),
        }
    }
    Ok((current, gone))
}

/// Groups to write, each with whether it is new.
pub(super) type Writes = Vec<(DirectoryGroup, bool)>;

/// The groups to write, split into those that take something away and those that add, and the ids
/// to remove. Equal groups are noted as unchanged.
pub(super) fn split_steps(
    steps: Vec<Change>,
    there: &BTreeMap<ObjectId, DirectoryGroup>,
    report: &mut SyncReport,
) -> (Writes, Writes, Vec<ObjectId>) {
    let (mut takes, mut gives, mut removals) = (vec![], vec![], vec![]);
    for step in steps {
        match step {
            Change::Keep(id) => report.unchanged.push(id),
            Change::Create(g) => gives.push((g, true)),
            Change::Update(g) => match there.get(&g.id) {
                Some(now) if takes_away(now, &g) => takes.push((g, false)),
                _ => gives.push((g, false)),
            },
            Change::Remove(id) => removals.push(id),
        }
    }
    (takes, gives, removals)
}

/// Whether writing `wanted` over `now` adds a person or a nested edge.
fn adds_anyone(now: Option<&DirectoryGroup>, wanted: &DirectoryGroup) -> bool {
    let Some(now) = now else {
        return !wanted.members.is_empty() || !wanted.nested.is_empty();
    };
    wanted.members.iter().any(|m| !now.members.contains(m))
        || wanted.nested.iter().any(|c| !now.nested.contains(c))
}

/// How far below a root each group is in the directory now (longest path), so that parents can be
/// written before their children. Bounded, so a loop in the directory cannot hang it.
fn depth(there: &BTreeMap<ObjectId, DirectoryGroup>) -> BTreeMap<ObjectId, usize> {
    let mut depth: BTreeMap<_, _> = there.keys().map(|id| (*id, 0usize)).collect();
    for _ in 0..there.len() {
        let mut moved = false;
        for (id, g) in there {
            let here = depth[id];
            for c in &g.nested {
                if let Some(d) = depth.get_mut(c).filter(|d| **d <= here) {
                    *d = here + 1;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    depth
}

/// Write the groups in an order that keeps the rule about agents true at every step.
///
/// What takes away goes first, parents before children (by the nesting the directory has now); what
/// adds follows, children before parents. A write that adds anybody is **held back** when a group
/// above it in the directory could not be updated (its edge to this group stays), and when it adds a
/// nested edge to a group that is not yet in its final state. A group that is held back or refused
/// blocks the adds below it in turn.
pub(super) async fn write_all(
    directory: &dyn GroupDirectory,
    mut takes: Writes,
    gives: Writes,
    there: &BTreeMap<ObjectId, DirectoryGroup>,
    present: &mut BTreeSet<ObjectId>,
    report: &mut SyncReport,
) {
    let rank = depth(there);
    takes.sort_by_key(|(g, _)| rank.get(&g.id).copied());
    let mut settled: BTreeSet<ObjectId> = report.unchanged.iter().copied().collect();
    let mut blocked = BTreeSet::new();
    for (g, new) in takes.into_iter().chain(gives) {
        let id = g.id;
        let now = there.get(&id);
        let new_children: Vec<_> = g
            .nested
            .iter()
            .filter(|c| !now.is_some_and(|n| n.nested.contains(c)))
            .collect();
        let early = new_children
            .iter()
            .any(|c| present.contains(c) && !settled.contains(c));
        if adds_anyone(now, &g) && (blocked.contains(&id) || early) {
            let why = "held back: a group above it that nests it, or a group it is to nest, is not in its final state";
            report.failed.push((id, why.to_string()));
            blocked.insert(id);
            blocked.extend(below(there, id));
        } else if put(directory, g, new, present, report).await {
            settled.insert(id);
        } else {
            blocked.insert(id);
            blocked.extend(below(there, id));
        }
    }
}
