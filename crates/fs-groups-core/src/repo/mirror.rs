//! The mirror side of the repository: the wanted directory state, and the ledger of what we mirrored.
//!
//! Pattern: Repository + Adapter — `desired` turns groups and people into `DirectoryGroup`s (nested
//! where the rights pass on, a flat list elsewhere);
//! the ledger is one store object per directory identity, so "only remove what we put there" survives restarts.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use fs_store::{ObjectId, Query, StoredObject};
use serde_json::json;

use super::consts::{AS2_TYPE, CONTEXT, LEDGER_TYPE};
use super::envelope::context;
use super::handle::GroupRepo;
use crate::error::Result;
use crate::mirror::{Divergence, Mirror};
use crate::model::Member;
use crate::right_flow;
use crate::traits::DirectoryGroup;

impl GroupRepo {
    async fn ledger(&self, directory: &str) -> Result<Option<StoredObject>> {
        let q = Query::new()
            .of_type(LEDGER_TYPE)
            .in_context(CONTEXT)
            .with_index("directory", directory);
        Ok(self.reads.query(&q).await?.into_iter().next())
    }
}

#[async_trait]
impl Mirror for GroupRepo {
    async fn desired(&self) -> Result<Vec<DirectoryGroup>> {
        let groups = self.all_groups().await?;
        let by_id: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g)).collect();
        let graph = crate::graph::Graph::from_groups(&groups);
        let flows = right_flow::settings(&groups);
        let mut people: BTreeMap<ObjectId, BTreeSet<String>> = BTreeMap::new();
        let all = self
            .memberships(Query::new().with_index("member_kind", "person"))
            .await?;
        for m in all {
            if let Member::Person(p) = m.member {
                people.entry(m.group).or_default().insert(p);
            }
        }
        let mut out = vec![];
        for id in graph.children_first() {
            let Some(g) = by_id.get(&id) else {
                continue;
            };
            // Option B (ADR 0056): a child is nested only where the rights really pass on; for any
            // other child the people of its whole subtree are listed in this group itself.
            let (nested, flat): (Vec<&ObjectId>, Vec<&ObjectId>) = graph
                .children(id)
                .iter()
                .partition(|c| right_flow::directory_expresses(&flows, **c, id));
            let mut members = people.get(&id).cloned().unwrap_or_default();
            for child in flat {
                for below in std::iter::once(*child).chain(graph.descendants(*child)) {
                    members.extend(people.get(&below).into_iter().flatten().cloned());
                }
            }
            out.push(
                DirectoryGroup {
                    id,
                    name: g.name.clone(),
                    members: members.into_iter().collect(),
                    nested: nested.into_iter().copied().collect(),
                }
                .normalised(),
            );
        }
        Ok(out)
    }

    async fn unreadable(&self) -> Result<Vec<ObjectId>> {
        Ok(self.read_groups().await?.1)
    }

    async fn divergences(&self) -> Result<Vec<Divergence>> {
        let groups = self.all_groups().await?;
        let graph = crate::graph::Graph::from_groups(&groups);
        let flows = right_flow::settings(&groups);
        let mut out: Vec<_> = groups
            .iter()
            .flat_map(|g| graph.parents(g.id).iter().map(|p| (g.id, *p)))
            .filter(|(child, parent)| !right_flow::directory_expresses(&flows, *child, *parent))
            .map(|(group, parent)| Divergence { group, parent })
            .collect();
        out.sort_by_key(|d| (d.group, d.parent));
        out.dedup();
        Ok(out)
    }

    async fn mirrored(&self, directory: &str) -> Result<BTreeSet<ObjectId>> {
        let ids = self
            .ledger(directory)
            .await?
            .and_then(|o| o.content.get("ids").cloned());
        let ids: Vec<String> = ids
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        Ok(ids.iter().filter_map(|s| s.parse().ok()).collect())
    }

    async fn remember(&self, directory: &str, ids: &BTreeSet<ObjectId>) -> Result<()> {
        let w = self.writer().await;
        let list: Vec<_> = ids.iter().map(ToString::to_string).collect();
        let content = json!({"@context": context(), "type": LEDGER_TYPE, "fs:directory": directory, "ids": list});
        let mut obj = match w.ledger(directory).await? {
            Some(mut o) => {
                o.content = content;
                o
            }
            None => StoredObject::new(
                w.fresh_envelope(LEDGER_TYPE, &format!("mirror ledger of {directory}")),
                AS2_TYPE,
                content,
            )
            .with_index("directory", directory),
        };
        obj.envelope.updated = chrono::Utc::now();
        w.put(obj).await
    }
}
