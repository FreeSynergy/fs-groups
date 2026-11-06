//! Memberships: add, remove, direct.
//!
//! Pattern: Repository — a membership is an AS2 `Relationship` in `org:Membership` form (subject =
//! the person, object = the group, `org:role`), with index fields for lookup.

use fs_store::{ObjectId, Order, Query};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, MEMBERSHIP_TYPE, ORG_MEMBERSHIP};
use super::envelope::{context, with_nonempty};
use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::model::{Member, Membership};

impl GroupRepo {
    /// `member.add`: add a person with a role (empty = plain member). Nesting groups is done with
    /// `add_parent`, so a group as member is refused. The same person with the same role twice is refused.
    pub async fn add(&self, group: ObjectId, member: Member, role: &str) -> Result<Membership> {
        let Member::Person(person) = &member else {
            return Err(Error::Conflict(
                "a group joins a group with add_parent, not as a member".into(),
            ));
        };
        if person.trim().is_empty() {
            return Err(Error::Conflict("a member needs an IRI".into()));
        }
        let g = self.require(group).await?;
        let role = role.trim();
        if self
            .direct(group)
            .await?
            .iter()
            .any(|m| m.member == member && m.role == role)
        {
            return Err(Error::Conflict(format!(
                "{person} is already in {} as {role:?}",
                g.name
            )));
        }
        let mut env = self.fresh_envelope(MEMBERSHIP_TYPE, &format!("{person} in {}", g.name));
        env.iri = Some(self.iri(env.id));
        let content = json!({
            "@context": context(), "type": MEMBERSHIP_TYPE, "subject": person,
            "relationship": ORG_MEMBERSHIP, "object": g.iri, "org:role": role,
        });
        let obj = fs_store::StoredObject::new(env, AS2_TYPE, content)
            .with_index("relationship", ORG_MEMBERSHIP)
            .with_index("group", group.to_string())
            .with_index("member_kind", member.kind())
            .with_index("member", member.key());
        let obj = with_nonempty(obj, "role", role);
        let id = obj.envelope.id;
        self.objects.put(obj, &self.actor).await?;
        Ok(Membership {
            id,
            group,
            member,
            role: role.into(),
        })
    }

    /// `member.remove`: end a membership.
    pub async fn remove(&self, membership: ObjectId) -> Result<()> {
        let found = self.objects.get(membership).await?;
        if found
            .as_ref()
            .is_none_or(|o| codec::membership(o).is_none())
        {
            return Err(Error::NotFound {
                what: "membership",
                id: membership,
            });
        }
        self.objects.delete(membership, &self.actor).await?;
        Ok(())
    }

    /// `member.read`: the direct memberships of a group, in the order they were made.
    pub async fn direct(&self, group: ObjectId) -> Result<Vec<Membership>> {
        self.require(group).await?;
        self.memberships(Query::new().with_index("group", group.to_string()))
            .await
    }

    /// Memberships matching `q` (type and context are added), oldest first.
    pub(super) async fn memberships(&self, q: Query) -> Result<Vec<Membership>> {
        let q = q
            .of_type(MEMBERSHIP_TYPE)
            .in_context(CONTEXT)
            .with_index("relationship", ORG_MEMBERSHIP)
            .ordered(Order::PublishedAsc);
        Ok(self
            .reads
            .query(&q)
            .await?
            .iter()
            .filter_map(codec::membership)
            .collect())
    }
}
