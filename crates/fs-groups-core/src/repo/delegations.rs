//! Delegations: grant, revoke, list, and the question `may_do`.
//!
//! Pattern: Repository + Specification — `Right::covers` decides whether a held right answers the
//! asked one; a right held by a group counts for every person who belongs to it, nesting included.
//! Nothing else enforces these rights yet (see `DESIGN.md`).

use fs_store::{ObjectId, Order, Query, StoredObject};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, DELEGATION_TYPE};
use super::envelope::context;
use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::model::{Delegation, Member, Right};

impl GroupRepo {
    /// `right.grant`: give a person or a group a right on `group`. Granting it twice returns the first.
    pub async fn grant(&self, group: ObjectId, holder: Member, right: Right) -> Result<Delegation> {
        let g = self.require(group).await?;
        match &holder {
            Member::Group(h) => {
                self.require(*h).await?;
            }
            Member::Person(p) if p.trim().is_empty() => {
                return Err(Error::Conflict("a holder needs an IRI".into()));
            }
            Member::Person(_) => {}
        }
        if let Some(same) = self
            .delegations(group)
            .await?
            .into_iter()
            .find(|d| d.holder == holder && d.right == right)
        {
            return Ok(same);
        }
        let env = self.fresh_envelope(
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
        self.objects.put(obj, &self.actor).await?;
        Ok(Delegation {
            id,
            group,
            holder,
            right,
        })
    }

    /// `right.grant`: take a delegation back.
    pub async fn revoke_right(&self, id: ObjectId) -> Result<()> {
        let found = self.objects.get(id).await?;
        if found
            .as_ref()
            .is_none_or(|o| codec::delegation(o).is_none())
        {
            return Err(Error::NotFound {
                what: "delegation",
                id,
            });
        }
        self.objects.delete(id, &self.actor).await?;
        Ok(())
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

    /// `right.read`: whether `person` holds `right` on `group`: directly, or through a group they belong
    /// to (nested groups count). `Manage` answers every right.
    pub async fn may_do(&self, group: ObjectId, person: &str, right: Right) -> Result<bool> {
        let held = self.delegations(group).await?;
        if held.iter().all(|d| !d.right.covers(right)) {
            return Ok(false);
        }
        let mine = self.groups_of(person).await?;
        Ok(held
            .iter()
            .filter(|d| d.right.covers(right))
            .any(|d| match &d.holder {
                Member::Person(p) => p == person,
                Member::Group(h) => mine.contains(h),
            }))
    }
}
