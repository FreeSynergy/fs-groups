//! Invitations: invite, accept, decline, revoke, expire, list.
//!
//! Pattern: Repository + State — an invitation is an AS2 `Invite`; every change of state goes through
//! `InvitationState::on`. Accepting creates the membership first, then marks the invitation, so a
//! failure in between is repaired by accepting again.

use fs_store::{ObjectId, Order, Query, StoredObject, Timestamp};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, INVITATION_TYPE};
use super::envelope::{context, with_nonempty};
use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::invitation_state::Step;
use crate::model::{Invitation, InvitationState, Member, Membership};

impl GroupRepo {
    /// `invite.create`: invite someone to a group; `expires` = `None` never runs out.
    pub async fn invite(
        &self,
        group: ObjectId,
        invitee: &str,
        role: &str,
        expires: Option<Timestamp>,
    ) -> Result<Invitation> {
        let g = self.require(group).await?;
        if invitee.trim().is_empty() {
            return Err(Error::Conflict("an invitation needs an invitee".into()));
        }
        let (invitee, role) = (invitee.trim(), role.trim());
        let mut env = self.fresh_envelope(
            INVITATION_TYPE,
            &format!("invitation of {invitee} to {}", g.name),
        );
        env.end_time = expires;
        let content = json!({
            "@context": context(), "type": INVITATION_TYPE, "object": g.iri, "target": invitee,
            "fs:role": role, "fs:state": InvitationState::Pending.as_str(),
        });
        let obj = StoredObject::new(env, AS2_TYPE, content)
            .with_index("group", group.to_string())
            .with_index("invitee", invitee)
            .with_index("state", InvitationState::Pending.as_str());
        let obj = with_nonempty(obj, "role", role);
        let id = obj.envelope.id;
        self.objects.put(obj, &self.actor).await?;
        Ok(Invitation {
            id,
            group,
            invitee: invitee.into(),
            role: role.into(),
            state: InvitationState::Pending,
            expires,
        })
    }

    async fn invitation(&self, id: ObjectId) -> Result<(StoredObject, Invitation)> {
        let o = self.objects.get(id).await?;
        o.and_then(|o| codec::invitation(&o).map(|i| (o, i)))
            .ok_or(Error::NotFound {
                what: "invitation",
                id,
            })
    }

    async fn set_state(&self, mut o: StoredObject, state: InvitationState) -> Result<()> {
        o.index.insert("state".into(), vec![state.as_str().into()]);
        o.content["fs:state"] = json!(state.as_str());
        self.objects.put(o, &self.actor).await?;
        Ok(())
    }

    /// Move an invitation by `step` (decline, revoke or expire); the expiry time does not matter for these.
    async fn step(&self, id: ObjectId, step: Step) -> Result<()> {
        let (o, inv) = self.invitation(id).await?;
        let next = inv.state.on(step, false)?;
        self.set_state(o, next).await
    }

    /// `invite.answer`: `person` accepts at `now`; this creates the membership (or finds it). An
    /// invitation that ran out is stored as `expired` and refused with [`Error::Expired`].
    pub async fn accept(&self, id: ObjectId, person: &str, now: Timestamp) -> Result<Membership> {
        let (o, inv) = self.invitation(id).await?;
        let next = inv.state.on(Step::Accept, inv.ran_out(now))?;
        if next == InvitationState::Expired {
            self.set_state(o, next).await?;
            return Err(Error::Expired(id));
        }
        let member = Member::Person(person.into());
        let held = self.direct(inv.group).await?;
        let membership = match held
            .into_iter()
            .find(|m| m.member == member && m.role == inv.role)
        {
            Some(m) => m,
            None => self.add(inv.group, member, &inv.role).await?,
        };
        self.set_state(o, next).await?;
        Ok(membership)
    }

    /// `invite.answer`: the invitee declines.
    pub async fn decline(&self, id: ObjectId) -> Result<()> {
        self.step(id, Step::Decline).await
    }

    /// `invite.create`: the group withdraws a pending invitation.
    pub async fn revoke(&self, id: ObjectId) -> Result<()> {
        self.step(id, Step::Revoke).await
    }

    /// `invite.answer`: mark every pending invitation that ran out by `now` as expired; how many.
    pub async fn expire_due(&self, now: Timestamp) -> Result<usize> {
        let q = Query::new()
            .of_type(INVITATION_TYPE)
            .in_context(CONTEXT)
            .with_index("state", InvitationState::Pending.as_str());
        let mut n = 0;
        for o in self.reads.query(&q).await? {
            if codec::invitation(&o).is_some_and(|i| i.ran_out(now)) {
                self.step(o.envelope.id, Step::Expire).await?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// `invite.read`: the invitations of a group, in any state, oldest first.
    pub async fn invitations(&self, group: ObjectId) -> Result<Vec<Invitation>> {
        self.require(group).await?;
        let q = Query::new()
            .of_type(INVITATION_TYPE)
            .in_context(CONTEXT)
            .with_index("group", group.to_string())
            .ordered(Order::PublishedAsc);
        Ok(self
            .reads
            .query(&q)
            .await?
            .iter()
            .filter_map(codec::invitation)
            .collect())
    }
}
