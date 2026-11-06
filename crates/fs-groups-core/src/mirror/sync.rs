//! [`Mirror`]: bring a directory to the state our groups say it should have.
//!
//! Pattern: Template method — `sync_to` is the fixed flow (remember intent, read, plan, apply,
//! remove, tidy the ledger);
//! what differs per program is only the small steps a provider implements.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use fs_store::ObjectId;

use super::plan::{Change, plan};
use super::report::{Divergence, SyncReport};
use super::wanted::{Scope, Wanted};
use crate::error::{Error, Result};
use crate::traits::{DirectoryGroup, GroupDirectory};

/// A source of groups that can be mirrored. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Mirror: Send + Sync {
    /// The groups as a directory should hold them, children before the groups that nest them.
    async fn desired(&self) -> Result<Vec<DirectoryGroup>>;
    /// The group ids we have put into the directory identified by `directory` before.
    async fn mirrored(&self, directory: &str) -> Result<BTreeSet<ObjectId>>;
    /// Stored groups and memberships that cannot be read, so `desired` may be missing people or
    /// groups. Default: none.
    async fn unreadable(&self) -> Result<Vec<ObjectId>> {
        Ok(vec![])
    }
    /// Groups on a loop (bad data from a second writer); their edges are never nested. Default: none.
    async fn looped(&self) -> Result<Vec<ObjectId>> {
        Ok(vec![])
    }
    /// `(group, people)` that `desired` leaves out of a group that carries an administrative right,
    /// because they are agents or not known to be human (ADR 0056). Default: none.
    async fn left_out(&self) -> Result<Vec<(ObjectId, Vec<String>)>> {
        Ok(vec![])
    }
    /// The edges of `desired` where the directory gives the child's people more than `may_do`.
    /// Only reported by `sync_to`; nothing is changed because of them. Default: none.
    async fn divergences(&self) -> Result<Vec<Divergence>> {
        Ok(vec![])
    }
    /// Everything above in one answer, worked out once. The default asks the single methods; if
    /// something is unreadable it holds still everywhere (`uncertain.all`).
    async fn wanted(&self) -> Result<Wanted> {
        let unreadable = self.unreadable().await?;
        Ok(Wanted {
            groups: self.desired().await?,
            left_out: self.left_out().await?,
            looped: self.looped().await?,
            uncertain: Scope {
                all: !unreadable.is_empty(),
                ..Scope::default()
            },
            unreadable,
            admin: Scope::default(),
        })
    }
    /// Whether an object with this id is still stored although it is not a group `desired` has
    /// (for example its type was changed). Such a group is never removed from a directory.
    async fn stored(&self, _id: ObjectId) -> Result<bool> {
        Ok(false)
    }
    /// The group to write for `group` while something that touches it cannot be read
    /// (`wanted.uncertain`): what is wanted plus what the directory holds now, so that nobody is
    /// taken out because of a gap. Also the people that had to be dropped. The default keeps
    /// everything; a source that knows administrative groups drops agents there.
    async fn settle(
        &self,
        group: DirectoryGroup,
        current: &BTreeMap<ObjectId, DirectoryGroup>,
        wanted: &Wanted,
    ) -> Result<(DirectoryGroup, Vec<String>)> {
        if wanted.uncertain.covers(group.id) && current.contains_key(&group.id) {
            Ok((hold_still(group, current), vec![]))
        } else {
            Ok((group, vec![]))
        }
    }
    /// Remember which ids are in the directory now.
    async fn remember(&self, directory: &str, ids: &BTreeSet<ObjectId>) -> Result<()>;

    /// Bring `directory` to the wanted state: create what is missing, repair what differs (a manual
    /// change is healed), remove what we mirrored and no longer have. Repeatable: a second run with
    /// nothing changed writes nothing. A group the directory refuses is reported and the rest goes on.
    ///
    /// The ledger (kept under [`GroupDirectory::directory_id`]) gets the **intent** before the
    /// directory is touched, so a failure later never leaves a group there that we forgot. An id
    /// leaves the ledger only when its group was removed or the directory confirms it is not there.
    /// A parent is applied only with the children the directory holds; the rest is reported.
    ///
    /// **Order.** Writes that take something away (people, a nested edge) go first, parents before
    /// children; then what adds, children first. So an agent is never written to a child while an
    /// administrative parent still nests it; if such a parent cannot be updated, the children below
    /// it that would gain people are held back and reported.
    async fn sync_to(&self, directory: &dyn GroupDirectory) -> Result<SyncReport> {
        let caps = directory.capabilities();
        if let Some(no) = caps.why_not("directory.apply") {
            return Err(no.into());
        }
        let name = directory.directory_id();
        let wanted = self.wanted().await?;
        let ids: BTreeSet<_> = wanted.groups.iter().map(|g| g.id).collect();
        let known = self.mirrored(&name).await?;
        let mut ledger = known.clone();
        ledger.extend(ids.iter().copied());
        let mut written = known.clone();
        if ledger != known {
            self.remember(&name, &ledger).await?;
            written.clone_from(&ledger);
        }
        let mut report = SyncReport {
            divergences: self.divergences().await?,
            unreadable: wanted.unreadable.clone(),
            looped: wanted.looped.clone(),
            ..SyncReport::default()
        };
        let (current, gone) = current_state(directory, &known, &ids, &mut report).await?;
        let removable: BTreeSet<_> = known
            .iter()
            .filter(|id| !wanted.unreadable.contains(id))
            .copied()
            .collect();
        let mut present: BTreeSet<_> = current.iter().map(|g| g.id).collect();
        let there: BTreeMap<_, _> = current
            .iter()
            .map(|g| (g.id, g.clone().normalised()))
            .collect();
        // where something unreadable touches a group, keep what is there (minus agents)
        let mut desired = vec![];
        let mut dropped_by_group = vec![];
        for g in &wanted.groups {
            let (g, dropped) = self.settle(g.clone(), &there, &wanted).await?;
            dropped_by_group.push((g.id, dropped));
            desired.push(g);
        }
        report.left_out = merge_left_out(&wanted.left_out, dropped_by_group);
        let order: BTreeMap<_, _> = desired.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
        let (mut takes, mut gives, removals) =
            split_steps(plan(&desired, &current, &removable), &there, &mut report);
        // parents before children for what takes away, children before parents for what adds
        takes.sort_by_key(|(g, _)| std::cmp::Reverse(order.get(&g.id).copied()));
        gives.sort_by_key(|(g, _)| order.get(&g.id).copied());
        let mut held_back = BTreeSet::new();
        for (g, new) in takes {
            let id = g.id;
            if !put(directory, g, new, &mut present, &mut report).await {
                held_back.extend(below(&there, id));
            }
        }
        for (g, new) in gives {
            if held_back.contains(&g.id) {
                let why =
                    "held back: a group above it that nests it could not be updated".to_string();
                report.failed.push((g.id, why));
                continue;
            }
            put(directory, g, new, &mut present, &mut report).await;
        }
        let mut removed = BTreeSet::new();
        for id in removals {
            // a stored object that is no longer a readable group stays where it is
            if self.stored(id).await? {
                report.unreadable.push(id);
                continue;
            }
            match directory.remove(id).await {
                Ok(()) => {
                    removed.insert(id);
                    report.removed.push(id);
                }
                Err(e) => report.failed.push((id, e.to_string())),
            }
        }
        report.unreadable.sort();
        report.unreadable.dedup();
        ledger.retain(|id| !removed.contains(id) && !gone.contains(id));
        if ledger != written {
            if let Err(e) = self.remember(&name, &ledger).await {
                report.ledger_error = Some(e.to_string());
            }
        }
        Ok(report)
    }
}

/// Whether writing `wanted` over `now` takes a person or a nested edge away.
fn takes_away(now: &DirectoryGroup, wanted: &DirectoryGroup) -> bool {
    let wanted = wanted.clone().normalised();
    now.members.iter().any(|m| !wanted.members.contains(m))
        || now.nested.iter().any(|c| !wanted.nested.contains(c))
}

/// The groups nested below `id` in the directory, at any depth.
fn below(there: &BTreeMap<ObjectId, DirectoryGroup>, id: ObjectId) -> BTreeSet<ObjectId> {
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
async fn put(
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
async fn current_state(
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
fn merge_left_out(
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
type Writes = Vec<(DirectoryGroup, bool)>;

/// The groups to write, split into those that take something away and those that add, and the ids
/// to remove. Equal groups are noted as unchanged.
fn split_steps(
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
