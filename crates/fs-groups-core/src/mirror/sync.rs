//! [`Mirror`]: bring a directory to the state our groups say it should have.
//!
//! Pattern: Template method — `sync_to` is the fixed flow (read, plan, apply, remove, remember);
//! what differs per program is only the three small steps a provider implements.

use std::collections::BTreeSet;

use async_trait::async_trait;
use fs_store::ObjectId;

use super::plan::{Change, plan};
use super::report::{Divergence, SyncReport};
use crate::error::Result;
use crate::traits::{DirectoryGroup, GroupDirectory};

/// A source of groups that can be mirrored. Interface status: draft (ADR 0045).
#[async_trait]
pub trait Mirror: Send + Sync {
    /// The groups as a directory should hold them, children before the groups that nest them.
    async fn desired(&self) -> Result<Vec<DirectoryGroup>>;
    /// The group ids we have put into the directory named `directory` before.
    async fn mirrored(&self, directory: &str) -> Result<BTreeSet<ObjectId>>;
    /// The nested edges of `desired` that the directory cannot hold as the groups' settings say.
    /// Only reported by `sync_to`; nothing is changed because of them. Default: none.
    async fn divergences(&self) -> Result<Vec<Divergence>> {
        Ok(vec![])
    }
    /// Remember which ids are in the directory now.
    async fn remember(&self, directory: &str, ids: &BTreeSet<ObjectId>) -> Result<()>;

    /// Bring `directory` to the wanted state: create what is missing, repair what differs (a manual
    /// change is healed), remove what we mirrored and no longer have. Repeatable: a second run with
    /// nothing changed writes nothing. A group the directory refuses is reported and the rest goes on.
    async fn sync_to(&self, directory: &dyn GroupDirectory) -> Result<SyncReport> {
        let caps = directory.capabilities();
        if let Some(no) = caps.why_not("directory.apply") {
            return Err(no.into());
        }
        let name = caps.provider().to_string();
        let desired = self.desired().await?;
        let wanted: BTreeSet<_> = desired.iter().map(|g| g.id).collect();
        let mut known = self.mirrored(&name).await?;
        let mut stuck = BTreeSet::new();
        let mut report = SyncReport {
            divergences: self.divergences().await?,
            ..SyncReport::default()
        };
        for step in plan(&desired, &directory.list().await?, &known) {
            match step {
                Change::Keep(id) => {
                    known.insert(id);
                    report.unchanged.push(id);
                }
                Change::Create(g) => put(directory, &g, true, &mut known, &mut report).await,
                Change::Update(g) => put(directory, &g, false, &mut known, &mut report).await,
                Change::Remove(id) => match directory.remove(id).await {
                    Ok(()) => report.removed.push(id),
                    Err(e) => {
                        stuck.insert(id);
                        report.failed.push((id, e.to_string()));
                    }
                },
            }
        }
        // What is neither wanted nor stuck is gone from the directory (or never got there).
        known.retain(|id| wanted.contains(id) || stuck.contains(id));
        self.remember(&name, &known).await?;
        Ok(report)
    }
}

async fn put(
    directory: &dyn GroupDirectory,
    group: &DirectoryGroup,
    new: bool,
    known: &mut BTreeSet<ObjectId>,
    report: &mut SyncReport,
) {
    match directory.apply(group).await {
        Ok(()) => {
            known.insert(group.id);
            if new {
                report.created.push(group.id);
            } else {
                report.updated.push(group.id);
            }
        }
        Err(e) => report.failed.push((group.id, e.to_string())),
    }
}
