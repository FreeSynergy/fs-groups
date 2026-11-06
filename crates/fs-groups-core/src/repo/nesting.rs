//! Nesting: parents, children, ancestors, descendants; a cycle is refused.
//!
//! Pattern: Specification (the no-cycle rule, checked on the in-memory `Graph` before anything is
//! written) inside a Repository. Kanidm would accept a loop (research note, section 3); we do not.

use fs_store::ObjectId;

use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::graph::Graph;
use crate::model::Group;

impl GroupRepo {
    pub(super) async fn graph(&self) -> Result<Graph> {
        Ok(Graph::from_groups(&self.all_groups().await?))
    }

    pub(super) async fn known_graph(&self, id: ObjectId) -> Result<Graph> {
        let g = self.graph().await?;
        if g.contains(id) {
            Ok(g)
        } else {
            Err(Error::NotFound { what: "group", id })
        }
    }

    /// `group.nest`: make `parent` a parent of `id`. Refused when `id` is `parent` or already one of
    /// its ancestors, with the chain that would close the loop. Adding a present link changes nothing.
    pub async fn add_parent(&self, id: ObjectId, parent: ObjectId) -> Result<Group> {
        let _write = self.lock.lock().await;
        let graph = self.known_graph(id).await?;
        if !graph.contains(parent) {
            return Err(Error::NotFound {
                what: "group",
                id: parent,
            });
        }
        if let Some(path) = graph.path_up(parent, id) {
            return Err(Error::Cycle {
                group: id,
                parent,
                path,
            });
        }
        let mut g = self.require(id).await?;
        if !g.parents.contains(&parent) {
            g.parents.push(parent);
            self.save(&g).await?;
        }
        Ok(g)
    }

    /// `group.nest`: end the link between `id` and `parent`. Removing an absent link changes nothing.
    pub async fn remove_parent(&self, id: ObjectId, parent: ObjectId) -> Result<Group> {
        let _write = self.lock.lock().await;
        let mut g = self.require(id).await?;
        if g.parents.contains(&parent) {
            g.parents.retain(|p| *p != parent);
            self.save(&g).await?;
        }
        Ok(g)
    }

    /// `group.read`: direct parents (only groups that exist).
    pub async fn parents(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(self.known_graph(id).await?.parents(id).to_vec())
    }

    /// `group.read`: direct children.
    pub async fn children(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(self.known_graph(id).await?.children(id).to_vec())
    }

    /// `group.read`: all ancestors, each once, nearest first.
    pub async fn ancestors(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(self.known_graph(id).await?.ancestors(id))
    }

    /// `group.read`: all descendants, each once, nearest first.
    pub async fn descendants(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(self.known_graph(id).await?.descendants(id))
    }
}
