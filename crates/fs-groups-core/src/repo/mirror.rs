//! The mirror side of the repository: the wanted directory state, and the ledger of what we mirrored.
//!
//! Pattern: Repository + Adapter — `desired` turns groups and direct people into `DirectoryGroup`s;
//! the ledger is one store object per directory name, so "only remove what we put there" survives restarts.

use std::collections::BTreeSet;

use async_trait::async_trait;
use fs_store::{ObjectId, Query, StoredObject};
use serde_json::json;

use super::consts::{AS2_TYPE, CONTEXT, LEDGER_TYPE};
use super::envelope::context;
use super::handle::GroupRepo;
use crate::error::Result;
use crate::mirror::Mirror;
use crate::model::Member;
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
        let graph = crate::graph::Graph::from_groups(&groups);
        let mut out = vec![];
        for id in graph.children_first() {
            let Some(g) = groups.iter().find(|g| g.id == id) else {
                continue;
            };
            let members = self
                .direct(id)
                .await?
                .into_iter()
                .filter_map(|m| match m.member {
                    Member::Person(p) => Some(p),
                    Member::Group(_) => None,
                });
            out.push(
                DirectoryGroup {
                    id,
                    name: g.name.clone(),
                    members: members.collect(),
                    nested: graph.children(id).to_vec(),
                }
                .normalised(),
            );
        }
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
        let list: Vec<_> = ids.iter().map(ToString::to_string).collect();
        let content = json!({"@context": context(), "type": LEDGER_TYPE, "fs:directory": directory, "ids": list});
        let mut obj = match self.ledger(directory).await? {
            Some(mut o) => {
                o.content = content;
                o
            }
            None => StoredObject::new(
                self.fresh_envelope(LEDGER_TYPE, &format!("mirror ledger of {directory}")),
                AS2_TYPE,
                content,
            )
            .with_index("directory", directory),
        };
        obj.envelope.updated = chrono::Utc::now();
        self.objects.put(obj, &self.actor).await?;
        Ok(())
    }
}
