//! The mirror side of the repository: the wanted directory state, and the ledger of what we mirrored.
//!
//! Pattern: Repository + Adapter — `desired` turns groups and people into `DirectoryGroup`s (nested
//! where the rights pass on, a flat list elsewhere);
//! the ledger is one store object per directory identity, so "only remove what we put there" survives restarts.

use std::collections::BTreeSet;

use async_trait::async_trait;
use fs_store::{ObjectId, Query, StoredObject};
use serde_json::json;

use super::consts::{AS2_TYPE, CONTEXT, LEDGER_TYPE};
use super::envelope::context;
use super::handle::GroupRepo;
use crate::error::Result;
use crate::mirror::{Divergence, Mirror};
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
        Ok(self.directory_state().await?.groups)
    }

    async fn unreadable(&self) -> Result<Vec<ObjectId>> {
        Ok(self.directory_state().await?.unreadable)
    }

    async fn looped(&self) -> Result<Vec<ObjectId>> {
        Ok(self.directory_state().await?.looped)
    }

    async fn left_out(&self) -> Result<Vec<(ObjectId, Vec<String>)>> {
        Ok(self.directory_state().await?.left_out)
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
