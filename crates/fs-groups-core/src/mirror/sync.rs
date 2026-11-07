//! [`Mirror`]: bring a directory to the state our groups say it should have.
//!
//! Pattern: Template method — `sync_to` is the fixed flow (remember intent, read, plan, apply,
//! remove, tidy the ledger);
//! what differs per program is only the small steps a provider implements.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use fs_objects::ObjectId;

use super::enforce::enforce;
use super::plan::plan;
use super::report::{Divergence, SyncReport};
use super::steps::hold_still;
use super::steps::{current_state, split_steps, write_all};
use super::wanted::{AdminRule, Scope, Verdict, Wanted};
use crate::error::Result;
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
    /// The edges of `desired` where the directory gives the child's people more than `may_do`.
    /// Only reported by `sync_to`; nothing is changed because of them. Default: none.
    async fn divergences(&self) -> Result<Vec<Divergence>> {
        Ok(vec![])
    }
    /// Everything above in one answer, worked out once. The default asks the single methods; if
    /// something is unreadable it holds still everywhere (`uncertain.all`). It knows no
    /// administrative group.
    async fn wanted(&self) -> Result<Wanted> {
        let unreadable = self.unreadable().await?;
        Ok(Wanted {
            groups: self.desired().await?,
            looped: self.looped().await?,
            uncertain: Scope {
                all: !unreadable.is_empty(),
                ..Scope::default()
            },
            unreadable,
            admin: AdminRule::default(),
        })
    }
    /// Whether an object with this id is still stored although it is not a group `desired` has
    /// (for example its type was changed). Such a group is never removed from a directory.
    async fn stored(&self, _id: ObjectId) -> Result<bool> {
        Ok(false)
    }
    /// The group to write for `group` while something that touches it cannot be read
    /// (`wanted.uncertain`): what is wanted plus what the directory holds now, so that nobody is
    /// taken out because of a gap. The rule about agents is not applied here but to the whole plan.
    async fn settle(
        &self,
        group: DirectoryGroup,
        current: &BTreeMap<ObjectId, DirectoryGroup>,
        wanted: &Wanted,
    ) -> Result<DirectoryGroup> {
        if wanted.uncertain.covers(group.id) {
            Ok(hold_still(group, current))
        } else {
            Ok(group)
        }
    }
    /// The host's verdict on each of `people`, for the rule that an agent is never admin. A person
    /// missing from the answer counts as unknown. Default: none (the default `wanted` has no
    /// administrative group).
    async fn verdicts(&self, _people: &BTreeSet<String>) -> Result<BTreeMap<String, Verdict>> {
        Ok(BTreeMap::new())
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
        // the final plan: what is wanted, held still where something is unreadable, then the rule
        // The plan is every group the directory will hold after the run: the ones we write, and
        // the ones we leave as they are (a group we cannot read is still nested in its parent).
        let mut target = there.clone();
        target.retain(|id, _| !removable.contains(id) || ids.contains(id));
        for g in &wanted.groups {
            let g = self.settle(g.clone(), &there, &wanted).await?.normalised();
            target.insert(g.id, g);
        }
        let people: BTreeSet<_> = target.values().flat_map(|g| g.members.clone()).collect();
        let verdicts = self.verdicts(&people).await?;
        let enforced = enforce(&mut target, &wanted.admin, &verdicts);
        report.left_out = enforced.left_out;
        report.unchecked = enforced.unchecked;
        let mut desired: Vec<_> = wanted
            .groups
            .iter()
            .map(|g| target[&g.id].clone())
            .collect();
        // a group we leave alone but the rule changed (an agent in a group we cannot read) is written too
        desired.extend(
            target
                .iter()
                .filter(|(id, g)| !ids.contains(id) && there.get(id) != Some(g))
                .map(|(_, g)| g.clone()),
        );
        let (takes, gives, removals) =
            split_steps(plan(&desired, &current, &removable), &there, &mut report);
        write_all(directory, takes, gives, &there, &mut present, &mut report).await;
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
