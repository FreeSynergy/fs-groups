//! Nesting: parents, children, ancestors, descendants; a cycle is refused.
//!
//! Pattern: Specification (the no-cycle rule, checked on the in-memory `Graph` before anything is
//! written) inside a Repository. Kanidm would accept a loop (research note, section 3); we do not.

use fs_objects::ObjectId;

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

    /// `group.nest`: end the link between `id` and `parent`. Removing an absent link changes nothing.
    /// Either side may end a link; nothing checks who asks.
    pub async fn remove_parent(&self, id: ObjectId, parent: ObjectId) -> Result<Group> {
        let w = self.writer().await;
        let mut g = w.require(id).await?;
        if !g.parents.contains(&parent) {
            return Ok(g);
        }
        g.parents.retain(|p| *p != parent);
        w.save(&g).await
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
