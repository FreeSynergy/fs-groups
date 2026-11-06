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
    /// groups. While there are any, `sync_to` removes nobody from a directory group. Default: none.
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
    async fn sync_to(&self, directory: &dyn GroupDirectory) -> Result<SyncReport> {
        let caps = directory.capabilities();
        if let Some(no) = caps.why_not("directory.apply") {
            return Err(no.into());
        }
        let name = directory.directory_id();
        let desired = self.desired().await?;
        let unreadable = self.unreadable().await?;
        let wanted: BTreeSet<_> = desired.iter().map(|g| g.id).collect();
        let known = self.mirrored(&name).await?;
        let mut ledger = known.clone();
        ledger.extend(wanted.iter().copied());
        let mut written = known.clone();
        if ledger != known {
            self.remember(&name, &ledger).await?;
            written.clone_from(&ledger);
        }
        let left_out = self.left_out().await?;
        let mut report = SyncReport {
            divergences: self.divergences().await?,
            unreadable: unreadable.clone(),
            looped: self.looped().await?,
            left_out: left_out.clone(),
            ..SyncReport::default()
        };
        let mut current = directory.list().await?;
        let mut gone = BTreeSet::new();
        // `list` may have left out a group of ours: ask for it before deciding it is gone.
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
        let removable: BTreeSet<_> = known
            .iter()
            .filter(|id| !unreadable.contains(id))
            .copied()
            .collect();
        let mut present: BTreeSet<_> = current.iter().map(|g| g.id).collect();
        let there: BTreeMap<_, _> = current
            .iter()
            .map(|g| (g.id, g.clone().normalised()))
            .collect();
        let keep_extra = !unreadable.is_empty();
        let mut removed = BTreeSet::new();
        for step in plan(&desired, &current, &removable) {
            match step {
                Change::Keep(id) => report.unchanged.push(id),
                Change::Create(g) => put(directory, g, true, &mut present, &mut report).await,
                Change::Update(g) => {
                    let g = if keep_extra {
                        // something cannot be read, so the wanted list may be short: take nobody out
                        let skipped = left_out.iter().find(|(id, _)| *id == g.id);
                        let now = there.get(&g.id).cloned();
                        keep_what_is_there(g, now.as_ref(), skipped.map(|(_, p)| p.as_slice()))
                    } else {
                        g
                    };
                    if there.get(&g.id) == Some(&g.clone().normalised()) {
                        report.unchanged.push(g.id);
                    } else {
                        put(directory, g, false, &mut present, &mut report).await;
                    }
                }
                Change::Remove(id) => match directory.remove(id).await {
                    Ok(()) => {
                        removed.insert(id);
                        report.removed.push(id);
                    }
                    Err(e) => report.failed.push((id, e.to_string())),
                },
            }
        }
        ledger.retain(|id| !removed.contains(id) && !gone.contains(id));
        if ledger != written {
            if let Err(e) = self.remember(&name, &ledger).await {
                report.ledger_error = Some(e.to_string());
            }
        }
        Ok(report)
    }
}

/// Apply one group with the children the directory holds, and record the outcome.
async fn put(
    directory: &dyn GroupDirectory,
    mut group: DirectoryGroup,
    new: bool,
    present: &mut BTreeSet<ObjectId>,
    report: &mut SyncReport,
) {
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
}

/// `wanted` plus what the directory holds now, except the people that must stay out.
fn keep_what_is_there(
    mut wanted: DirectoryGroup,
    now: Option<&DirectoryGroup>,
    never: Option<&[String]>,
) -> DirectoryGroup {
    if let Some(now) = now {
        wanted.members.extend(
            now.members
                .iter()
                .filter(|p| !never.is_some_and(|n| n.contains(p)))
                .cloned(),
        );
        wanted.nested.extend(now.nested.iter().copied());
    }
    wanted.normalised()
}
