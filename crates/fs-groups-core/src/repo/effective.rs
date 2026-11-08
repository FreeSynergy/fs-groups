//! Effective membership: through nested groups, with the path.
//!
//! Pattern: Visitor over the in-memory `Graph` — a person in a child belongs to every ancestor.
//! The walk visits each group once, so it ends on any data.

use std::collections::BTreeSet;

use fs_objects::{ObjectId, Query};

use super::handle::GroupRepo;
use crate::error::Result;
use crate::model::{EffectiveMember, Member};

impl GroupRepo {
    /// `member.read`: everyone who belongs to `group`, nested groups included. Each person once, with
    /// the shortest path: `[group]` for a direct member, `[group, child, ...]` further down.
    pub async fn effective(&self, group: ObjectId) -> Result<Vec<EffectiveMember>> {
        let graph = self.known_graph(group).await?;
        let mut nodes = vec![(group, vec![group])];
        nodes.extend(graph.descendants_with_paths(group));
        let mut seen = BTreeSet::new();
        let mut out = vec![];
        for (node, path) in nodes {
            for m in self.direct(node).await? {
                if let Member::Person(p) = m.member {
                    if seen.insert(p.clone()) {
                        out.push(EffectiveMember {
                            person: p,
                            via: path.clone(),
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// `member.read`: the groups a person belongs to: the ones they are in, and all their ancestors.
    pub async fn groups_of(&self, person: &str) -> Result<Vec<ObjectId>> {
        let person = person.trim();
        let graph = self.graph().await?;
        let q = Query::new()
            .with_index("member_kind", "person")
            .with_index("member", person);
        let mut all = BTreeSet::new();
        for m in self
            .memberships(q)
            .await?
            .into_iter()
            .filter(|m| graph.contains(m.group))
        {
            all.insert(m.group);
            all.extend(graph.ancestors(m.group));
        }
        Ok(all.into_iter().collect())
    }

    /// `member.read`: whether `person` belongs to `group`, through nesting too.
    pub async fn is_member(&self, group: ObjectId, person: &str) -> Result<bool> {
        self.require(group).await?;
        Ok(self.groups_of(person).await?.contains(&group))
    }
}
