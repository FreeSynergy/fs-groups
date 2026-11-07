//! Delegations: grant, revoke, list, and the question `may_do`.
//!
//! Pattern: Repository + Specification — `Right::covers` decides whether a held right answers the
//! asked one; where a right applies and who receives it are two clauses of `right_flow` (the safe,
//! provisional reading). Nothing else enforces these rights yet (see `DESIGN.md`).
//! In doubt the answer is no: an unknown group is an error, unreadable or looped data never grants.

use std::collections::BTreeSet;

use fs_objects::{ObjectId, Order, Query, StoredObject};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, DELEGATION_TYPE};
use super::envelope::context;
use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::graph::Graph;
use crate::model::{Delegation, Member, Right};
use crate::right_flow;

impl GroupRepo {
    /// `right.grant`: give a person or a group a right on `group`. Granting the same right to the same
    /// holder again returns the first delegation. A holder group must exist.
    pub async fn grant(&self, group: ObjectId, holder: Member, right: Right) -> Result<Delegation> {
        let holder = match holder {
            Member::Person(p) if p.trim().is_empty() => {
                return Err(Error::Invalid("a holder needs an IRI".into()));
            }
            Member::Person(p) => Member::Person(p.trim().into()),
            group_holder @ Member::Group(_) => group_holder,
        };
        let w = self.writer().await;
        let g = w.require(group).await?;
        if let Member::Group(h) = &holder {
            w.require(*h).await?;
        }
        if let Some(same) = w
            .delegations(group)
            .await?
            .into_iter()
            .find(|d| d.holder == holder && d.right == right)
        {
            return Ok(same);
        }
        let env = w.fresh_envelope(
            DELEGATION_TYPE,
            &format!("{} may {} {}", holder.key(), right.as_str(), g.name),
        );
        let content = json!({
            "@context": context(), "type": DELEGATION_TYPE, "object": g.iri,
            "subject": holder.key(), "fs:right": right.as_str(),
        });
        let obj = StoredObject::new(env, AS2_TYPE, content)
            .with_index("group", group.to_string())
            .with_index("holder_kind", holder.kind())
            .with_index("holder", holder.key())
            .with_index("right", right.as_str());
        let id = obj.envelope.id;
        w.put(obj).await?;
        Ok(Delegation {
            id,
            group,
            holder,
            right,
        })
    }

    /// `right.grant`: take a delegation back.
    pub async fn revoke_right(&self, id: ObjectId) -> Result<()> {
        let w = self.writer().await;
        let found = w.load(id).await?;
        if found
            .as_ref()
            .is_none_or(|o| codec::delegation(o).is_none())
        {
            return Err(Error::NotFound {
                what: "delegation",
                id,
            });
        }
        w.remove_object(id).await
    }

    /// `right.read`: the delegations of a group, oldest first.
    pub async fn delegations(&self, group: ObjectId) -> Result<Vec<Delegation>> {
        self.require(group).await?;
        let q = Query::new()
            .of_type(DELEGATION_TYPE)
            .in_context(CONTEXT)
            .with_index("group", group.to_string())
            .ordered(Order::PublishedAsc);
        Ok(self
            .reads
            .query(&q)
            .await?
            .iter()
            .filter_map(codec::delegation)
            .collect())
    }

    /// `right.read`: whether `person` holds `right` on `group` (`Manage` answers every right).
    ///
    /// A delegation counts if it sits on a group where `right_flow::sources` says its right applies to
    /// `group`, and the person is its holder or a **direct** member of one of the holder group's
    /// `right_flow::recipients`. Unknown group: [`Error::NotFound`]. A group on a loop, a holder group
    /// that is gone, an object that cannot be read, an unknown setting (read as `none`): never a yes.
    /// A failing read is an error, not a no. The graph is loaded once per call.
    pub async fn may_do(&self, group: ObjectId, person: &str, right: Right) -> Result<bool> {
        let person = person.trim();
        let groups = self.all_groups().await?;
        let graph = Graph::from_groups(&groups);
        if !graph.contains(group) {
            return Err(Error::NotFound {
                what: "group",
                id: group,
            });
        }
        let cyclic = graph.cyclic_nodes();
        if person.is_empty() || cyclic.contains(&group) {
            return Ok(false);
        }
        let flows = right_flow::settings(&groups);
        let sources: BTreeSet<_> = right_flow::sources(&graph, &flows, group)
            .into_iter()
            .filter(|s| !cyclic.contains(s))
            .collect();
        let q = Query::new()
            .with_index("member_kind", "person")
            .with_index("member", person);
        let mine: BTreeSet<_> = self.memberships(q).await?.iter().map(|m| m.group).collect();
        let mut asked = vec![right];
        if right != Right::Manage {
            asked.push(Right::Manage);
        }
        for held in asked {
            let q = Query::new()
                .of_type(DELEGATION_TYPE)
                .in_context(CONTEXT)
                .with_index("right", held.as_str());
            for d in self
                .reads
                .query(&q)
                .await?
                .iter()
                .filter_map(codec::delegation)
            {
                if !sources.contains(&d.group) || !d.right.covers(right) {
                    continue;
                }
                let held = match &d.holder {
                    Member::Person(p) => p == person,
                    Member::Group(h) => {
                        !cyclic.contains(h)
                            && right_flow::recipients(&flows, *h)
                                .iter()
                                .any(|g| mine.contains(g))
                    }
                };
                if held {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
