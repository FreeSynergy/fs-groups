//! Memberships: add, remove, direct.
//!
//! Pattern: Repository — a membership is an AS2 `Relationship` in `org:Membership` form (subject =
//! the person, object = the group, `org:role`), with index fields for lookup.

use fs_objects::{ObjectId, Order, Query};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, MEMBERSHIP_TYPE, ORG_MEMBERSHIP};
use super::envelope::{context, with_nonempty};
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::model::{Member, Membership};
use crate::text;

impl GroupRepo {
    /// `member.add`: add a person with a role (empty = plain member). Nesting groups is done with
    /// `request_parent`, so a group as member is refused. The same person with the same role twice is
    /// one membership: the second call returns the first. The person is stored trimmed.
    pub async fn add(&self, group: ObjectId, member: Member, role: &str) -> Result<Membership> {
        let Member::Person(person) = &member else {
            return Err(Error::Invalid(
                "a group joins a group with request_parent, not as a member".into(),
            ));
        };
        if person.trim().is_empty() {
            return Err(Error::Invalid("a member needs an IRI".into()));
        }
        text::check_role("role", role.trim())?;
        let member = Member::Person(text::person("person", person)?);
        self.writer()
            .await
            .add_member(group, member, role.trim())
            .await
    }

    /// `member.remove`: end a membership.
    pub async fn remove(&self, membership: ObjectId) -> Result<()> {
        let w = self.writer().await;
        let found = w.load(membership).await?;
        // an unreadable membership can be ended too, or it could never be cleaned up
        if found.as_ref().is_none_or(|o| {
            o.envelope.object_type != MEMBERSHIP_TYPE
                || o.envelope.context.as_deref() != Some(CONTEXT)
        }) {
            return Err(Error::NotFound {
                what: "membership",
                id: membership,
            });
        }
        w.remove_object(membership).await
    }

    /// `member.read`: the direct memberships of a group, in the order they were made.
    pub async fn direct(&self, group: ObjectId) -> Result<Vec<Membership>> {
        self.require(group).await?;
        let mut found = self
            .memberships(Query::new().with_index("group", group.to_string()))
            .await?;
        for m in &mut found {
            if let (true, Member::Person(p)) = (self.is_admin_role(&m.role), &m.member) {
                // a checker that fails must not block reads: the entry just does not show as in force
                m.in_force = self.is_known_human(p).await.unwrap_or(false);
            }
        }
        Ok(found)
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

impl Writer<'_> {
    /// The membership of `member` (a trimmed person) in `group` with `role`, made if it is not there.
    pub(super) async fn add_member(
        &self,
        group: ObjectId,
        member: Member,
        role: &str,
    ) -> Result<Membership> {
        let g = self.require(group).await?;
        if let (true, Member::Person(person)) = (self.is_admin_role(role), &member) {
            self.require_human(person, &format!("role {role:?} in {}", g.name))
                .await?;
        }
        if let Some(same) = self
            .direct(group)
            .await?
            .into_iter()
            .find(|m| m.member == member && m.role == role)
        {
            return Ok(same);
        }
        let mut env =
            self.fresh_envelope(MEMBERSHIP_TYPE, &format!("{} in {}", member.key(), g.name));
        env.iri = Some(self.iri(env.id));
        let content = json!({
            "@context": context(), "type": MEMBERSHIP_TYPE, "subject": member.key(),
            "relationship": ORG_MEMBERSHIP, "object": g.iri, "org:role": role,
        });
        let obj = fs_objects::StoredObject::new(env, AS2_TYPE, content)
            .with_index("relationship", ORG_MEMBERSHIP)
            .with_index("group", group.to_string())
            .with_index("member_kind", member.kind())
            .with_index("member", member.key());
        let obj = with_nonempty(obj, "role", role);
        let id = obj.envelope.id;
        self.put(obj).await?;
        Ok(Membership {
            id,
            group,
            member,
            role: role.into(),
            in_force: true,
        })
    }
}
