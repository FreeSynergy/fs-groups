//! Invitations: invite, accept, decline, revoke, expire, list.
//!
//! Pattern: Repository + State — an invitation is an AS2 `Invite`; every change of state goes through
//! `InvitationState::on`. Every step reads the invitation and writes it back under one lock, so two
//! answers cannot both win. Accepting creates the membership first, then marks the invitation, so a
//! failure in between is repaired by accepting again.

use fs_store::{ObjectId, Order, Query, StoredObject, Timestamp};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, INVITATION_TYPE};
use super::envelope::{context, with_nonempty};
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::invitation_state::Step;
use crate::model::{Invitation, InvitationState, Member, Membership};
use crate::text;

impl GroupRepo {
    /// `invite.create`: invite someone; `expires` = `None` never runs out.
    pub async fn invite(
        &self,
        group: ObjectId,
        invitee: &str,
        role: &str,
        expires: Option<Timestamp>,
    ) -> Result<Invitation> {
        if invitee.trim().is_empty() {
            return Err(Error::Conflict("an invitation needs an invitee".into()));
        }
        let (invitee, role) = (invitee.trim(), role.trim());
        text::check_person("invitee", invitee)?;
        text::check_role("role", role)?;
        let w = self.writer().await;
        let g = w.require(group).await?;
        let mut env = w.fresh_envelope(
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
        w.put(obj).await?;
        Ok(Invitation {
            id,
            group,
            invitee: invitee.into(),
            role: role.into(),
            state: InvitationState::Pending,
            expires,
        })
    }

    /// `invite.answer`: move an invitation by `step` (decline, revoke or expire); the expiry time does
    /// not matter for these. Read and write happen under one lock.
    async fn step(&self, id: ObjectId, step: Step) -> Result<()> {
        let w = self.writer().await;
        let (o, inv) = w.invitation(id).await?;
        let next = inv.state.on(step, false)?;
        w.set_state(o, next).await
    }

    /// `invite.answer`: `person` accepts at `now`; this creates the membership (or finds it).
    ///
    /// **Only the invitee can accept.** `person` must be the invitee exactly. If the invitation went
    /// to an e-mail address, a person (an IRI) cannot be compared with it here: use
    /// [`GroupRepo::accept_proved`]. A wrong person gets [`Error::NotInvitee`] and the invitation
    /// stays as it is. An invitation that ran out is stored as `expired` and refused with
    /// [`Error::Expired`].
    pub async fn accept(&self, id: ObjectId, person: &str, now: Timestamp) -> Result<Membership> {
        self.answer_accept(id, person, None, now).await
    }

    /// `invite.answer`: like [`GroupRepo::accept`], for an invitation sent to an e-mail address.
    /// **Provisional.** The caller (the server, with fs-people) says which verified address `person`
    /// proved control of; it is compared with the invitee ignoring case. This crate cannot verify
    /// anything: whoever calls this with an address vouches for it. Whether a secret token in the
    /// invitation replaces this is open.
    pub async fn accept_proved(
        &self,
        id: ObjectId,
        person: &str,
        proved_address: &str,
        now: Timestamp,
    ) -> Result<Membership> {
        self.answer_accept(id, person, Some(proved_address), now)
            .await
    }

    async fn answer_accept(
        &self,
        id: ObjectId,
        person: &str,
        proved: Option<&str>,
        now: Timestamp,
    ) -> Result<Membership> {
        let person = person.trim();
        if person.is_empty() {
            return Err(Error::Conflict("an accepting person needs an IRI".into()));
        }
        text::check_person("person", person)?;
        let w = self.writer().await;
        let (o, inv) = w.invitation(id).await?;
        let next = inv.state.on(Step::Accept, inv.ran_out(now))?;
        if !inv.is_for(person, proved) {
            return Err(Error::NotInvitee(id));
        }
        if next == InvitationState::Expired {
            w.set_state(o, next).await?;
            return Err(Error::Expired(id));
        }
        let membership = w
            .add_member(inv.group, Member::Person(person.into()), &inv.role)
            .await?;
        w.set_state(o, next).await?;
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
    /// One answered in the meantime is skipped.
    pub async fn expire_due(&self, now: Timestamp) -> Result<usize> {
        let q = Query::new()
            .of_type(INVITATION_TYPE)
            .in_context(CONTEXT)
            .with_index("state", InvitationState::Pending.as_str());
        let mut n = 0;
        for o in self.reads.query(&q).await? {
            if codec::invitation(&o).is_some_and(|i| i.ran_out(now)) {
                match self.step(o.envelope.id, Step::Expire).await {
                    Ok(()) => n += 1,
                    Err(Error::InvalidTransition { .. }) => {}
                    Err(e) => return Err(e),
                }
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

impl Writer<'_> {
    async fn invitation(&self, id: ObjectId) -> Result<(StoredObject, Invitation)> {
        let o = self.load(id).await?;
        o.and_then(|o| codec::invitation(&o).map(|i| (o, i)))
            .ok_or(Error::NotFound {
                what: "invitation",
                id,
            })
    }

    async fn set_state(&self, mut o: StoredObject, state: InvitationState) -> Result<()> {
        o.index.insert("state".into(), vec![state.as_str().into()]);
        o.content["fs:state"] = json!(state.as_str());
        self.put(o).await
    }
}
