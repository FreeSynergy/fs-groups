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

/// The people left out of each group, from the source and from the groups that were held still.
pub(super) fn merge_left_out(
    from_source: &[(ObjectId, Vec<String>)],
    dropped: Vec<(ObjectId, Vec<String>)>,
) -> Vec<(ObjectId, Vec<String>)> {
    let mut merged: BTreeMap<ObjectId, BTreeSet<String>> = BTreeMap::new();
    for (id, people) in from_source.iter().cloned().chain(dropped) {
        merged.entry(id).or_default().extend(people);
    }
    merged
        .into_iter()
        .filter(|(_, people)| !people.is_empty())
        .map(|(id, people)| (id, people.into_iter().collect()))
        .collect()
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
