//! [`Mirror`]: bring a directory to the state our groups say it should have.
//!
//! Pattern: Template method — `sync_to` is the fixed flow (remember intent, read, plan, apply,
//! remove, tidy the ledger);
//! what differs per program is only the small steps a provider implements.

use std::collections::BTreeSet;

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
    /// Stored groups that cannot be read, so they are not in `desired`. They are never removed from
    /// a directory because of that. Default: none.
    async fn unreadable(&self) -> Result<Vec<ObjectId>> {
        Ok(vec![])
    }
    /// The nested edges of `desired` where the directory gives more than the groups' rights do.
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
        if ledger != known {
            self.remember(&name, &ledger).await?;
        }
        let mut report = SyncReport {
            divergences: self.divergences().await?,
            unreadable: unreadable.clone(),
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
        let mut removed = BTreeSet::new();
        for step in plan(&desired, &current, &removable) {
            match step {
                Change::Keep(id) => report.unchanged.push(id),
                Change::Create(g) => put(directory, g, true, &mut present, &mut report).await,
                Change::Update(g) => put(directory, g, false, &mut present, &mut report).await,
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
        if let Err(e) = self.remember(&name, &ledger).await {
            report.ledger_error = Some(e.to_string());
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
