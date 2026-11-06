//! Stored object -> typed value for memberships, invitations and delegations.
//!
//! Pattern: Adapter — the ids and states are read from the index fields written next to the AS2
//! content (the content is the standard-shaped view); a malformed object reads as `None`.

use fs_store::{ObjectId, StoredObject};

use crate::model::{
    Delegation, Invitation, InvitationState, Member, Membership, ParentRequest, RequestState, Right,
};

fn one<'a>(o: &'a StoredObject, key: &str) -> Option<&'a str> {
    o.index.get(key)?.first().map(String::as_str)
}

fn id_of(o: &StoredObject, key: &str) -> Option<ObjectId> {
    one(o, key)?.parse().ok()
}

fn member_of(o: &StoredObject, kind: &str, key: &str) -> Option<Member> {
    Member::from_parts(one(o, kind)?, one(o, key)?)
}

pub(super) fn membership(o: &StoredObject) -> Option<Membership> {
    Some(Membership {
        id: o.envelope.id,
        group: id_of(o, "group")?,
        member: member_of(o, "member_kind", "member")?,
        // the index is what is searched; if it lost the role, the standard-shaped content still has it
        role: one(o, "role")
            .or_else(|| o.content.get("org:role")?.as_str())
            .unwrap_or_default()
            .to_string(),
        in_force: true,
    })
}

pub(super) fn invitation(o: &StoredObject) -> Option<Invitation> {
    Some(Invitation {
        id: o.envelope.id,
        group: id_of(o, "group")?,
        invitee: one(o, "invitee")?.to_string(),
        role: one(o, "role").unwrap_or_default().to_string(),
        state: InvitationState::parse(one(o, "state")?)?,
        expires: o.envelope.end_time,
    })
}

pub(super) fn delegation(o: &StoredObject) -> Option<Delegation> {
    Some(Delegation {
        id: o.envelope.id,
        group: id_of(o, "group")?,
        holder: member_of(o, "holder_kind", "holder")?,
        right: Right::parse(one(o, "right").or_else(|| o.content.get("fs:right")?.as_str())?)?,
        in_force: true,
    })
}

pub(super) fn parent_request(o: &StoredObject) -> Option<ParentRequest> {
    Some(ParentRequest {
        id: o.envelope.id,
        child: id_of(o, "child")?,
        parent: id_of(o, "parent_of")?,
        requester: o.envelope.attributed_to.clone().unwrap_or_default(),
        state: RequestState::parse(one(o, "state")?)?,
    })
}
