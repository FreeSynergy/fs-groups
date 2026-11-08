//! Stored object -> typed value for memberships, invitations and delegations.
//!
//! Pattern: Adapter — the ids and states are read from the index fields written next to the AS2
//! content (the content is the standard-shaped view); a malformed object reads as `None`.

use fs_objects::{ObjectId, StoredObject};

use crate::model::{
    Delegation, Invitation, InvitationState, Member, Membership, ParentRequest, RequestState, Right,
};

pub(super) fn one<'a>(o: &'a StoredObject, key: &str) -> Option<&'a str> {
    o.index.get(key)?.first().map(String::as_str)
}

pub(super) fn id_of(o: &StoredObject, key: &str) -> Option<ObjectId> {
    one(o, key)?.parse().ok()
}

fn member_of(o: &StoredObject, kind: &str, key: &str) -> Option<Member> {
    Member::from_parts(one(o, kind)?, one(o, key)?)
}

/// A text member of the AS2 content.
fn content<'a>(o: &'a StoredObject, key: &str) -> Option<&'a str> {
    o.content.get(key)?.as_str()
}

/// The group id at the end of a group IRI (`https://<instance>/groups/<id>`).
fn group_in(iri: &str) -> Option<ObjectId> {
    iri.rsplit('/').next()?.parse().ok()
}

/// A membership, read from the index fields **and checked against the AS2 content**, which is the
/// authoritative form: group, person and role must agree. Where they do not, someone edited one of
/// them by hand and the object is unreadable (the caller reports it, and fails closed).
pub(super) fn membership(o: &StoredObject) -> Option<Membership> {
    let group = id_of(o, "group")?;
    let member = member_of(o, "member_kind", "member")?;
    let role = one(o, "role").unwrap_or_default();
    let agrees = group_in(content(o, "object")?) == Some(group)
        && content(o, "subject")? == member.key()
        && content(o, "org:role").unwrap_or_default() == role;
    agrees.then(|| Membership {
        id: o.envelope.id,
        group,
        member,
        role: role.to_string(),
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

/// A delegation, read from the index and checked against the AS2 content (see [`membership`]).
pub(super) fn delegation(o: &StoredObject) -> Option<Delegation> {
    let group = id_of(o, "group")?;
    let holder = member_of(o, "holder_kind", "holder")?;
    let right = Right::parse(one(o, "right")?)?;
    let agrees = group_in(content(o, "object")?) == Some(group)
        && content(o, "subject")? == holder.key()
        && content(o, "fs:right")? == right.as_str();
    agrees.then_some(Delegation {
        id: o.envelope.id,
        group,
        holder,
        right,
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
