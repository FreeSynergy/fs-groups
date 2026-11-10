//! Records read from the wire.
//!
//! Pattern: Adapter — Cap'n Proto readers of `groups.capnp` to the frontend's `Record` (texts by field name). A field
//! without a value is left out, so the frontend shows it empty.

use fs_admin_frontend::{Record, Values};
use fs_interfaces::common_capnp::uuid;
use fs_interfaces::groups_capnp::{
    DeliveryState, InvitationState, RequestState, SourceState, delegation, delivery, group,
    invitation, member, membership, parent_request, source,
};
use fs_interfaces::rpc::{from_millis, read_envelope, read_uuid};

use super::input::{flow_key, right_key, yes_no};
use crate::remote::RunError;

fn put(v: &mut Values, field: &str, value: impl Into<String>) {
    let value = value.into();
    if !value.is_empty() {
        v.insert(field.to_owned(), vec![value]);
    }
}

fn put_all(v: &mut Values, field: &str, values: Vec<String>) {
    if !values.is_empty() {
        v.insert(field.to_owned(), values);
    }
}

/// The id as text.
pub fn id_text(r: capnp::Result<uuid::Reader<'_>>) -> Result<String, RunError> {
    Ok(read_uuid(r?).map(|u| u.to_string()).unwrap_or_default())
}

fn ids(
    list: capnp::Result<capnp::struct_list::Reader<'_, uuid::Owned>>,
) -> Result<Vec<String>, RunError> {
    Ok(list?
        .iter()
        .filter_map(read_uuid)
        .map(|u| u.to_string())
        .collect())
}

/// The ids of a list of ids.
pub fn id_list(
    list: capnp::Result<capnp::struct_list::Reader<'_, uuid::Owned>>,
) -> Result<Vec<String>, RunError> {
    ids(list)
}

fn text(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<String, RunError> {
    Ok(t?.to_str()?.to_owned())
}

fn who(m: capnp::Result<member::Reader<'_>>) -> Result<String, RunError> {
    Ok(match m?.which()? {
        member::Which::Person(p) => text(p)?,
        member::Which::Group(g) => id_text(g)?,
    })
}

/// A group.
pub fn group(g: group::Reader<'_>) -> Result<Record, RunError> {
    let env = read_envelope(g.get_envelope()?)?;
    let mut v = Values::new();
    put(&mut v, "name", env.name);
    put(&mut v, "kind", text(g.get_kind())?);
    put_all(&mut v, "parents", ids(g.get_parents())?);
    put(&mut v, "rightFlow", flow_key(g.get_right_flow()?));
    Ok(Record {
        id: env.id.to_string(),
        values: v,
    })
}

/// A direct membership.
pub fn membership(m: membership::Reader<'_>) -> Result<Record, RunError> {
    let mut v = Values::new();
    put(&mut v, "group", id_text(m.get_group())?);
    put(&mut v, "member", who(m.get_member())?);
    put(&mut v, "role", text(m.get_role())?);
    put(&mut v, "inForce", yes_no(m.get_in_force()));
    Ok(Record {
        id: id_text(m.get_id())?,
        values: v,
    })
}

/// An invitation.
pub fn invitation(i: invitation::Reader<'_>) -> Result<Record, RunError> {
    let mut v = Values::new();
    put(&mut v, "group", id_text(i.get_group())?);
    put(&mut v, "invitee", text(i.get_invitee())?);
    put(&mut v, "role", text(i.get_role())?);
    put(
        &mut v,
        "state",
        match i.get_state()? {
            InvitationState::Pending => "pending",
            InvitationState::Accepted => "accepted",
            InvitationState::Declined => "declined",
            InvitationState::Revoked => "revoked",
            InvitationState::Expired => "expired",
        },
    );
    if i.has_expires() {
        let at = from_millis(i.get_expires()?.get_unix_millis())?;
        put(&mut v, "expires", at.to_rfc3339());
    }
    Ok(Record {
        id: id_text(i.get_id())?,
        values: v,
    })
}

/// A delegation.
pub fn delegation(d: delegation::Reader<'_>) -> Result<Record, RunError> {
    let mut v = Values::new();
    put(&mut v, "group", id_text(d.get_group())?);
    put(&mut v, "holder", who(d.get_holder())?);
    put(&mut v, "right", right_key(d.get_right()?));
    put(&mut v, "inForce", yes_no(d.get_in_force()));
    Ok(Record {
        id: id_text(d.get_id())?,
        values: v,
    })
}

/// A request to become a child.
pub fn parent_request(r: parent_request::Reader<'_>) -> Result<Record, RunError> {
    let mut v = Values::new();
    put(&mut v, "child", id_text(r.get_child())?);
    put(&mut v, "parent", id_text(r.get_parent())?);
    put(&mut v, "requester", text(r.get_requester())?);
    put(
        &mut v,
        "state",
        match r.get_state()? {
            RequestState::Pending => "pending",
            RequestState::Approved => "approved",
            RequestState::Declined => "declined",
        },
    );
    Ok(Record {
        id: id_text(r.get_id())?,
        values: v,
    })
}

fn rights_of(
    list: capnp::Result<capnp::enum_list::Reader<'_, fs_interfaces::groups_capnp::Right>>,
) -> Result<Vec<String>, RunError> {
    list?.iter().map(|r| Ok(right_key(r?).to_owned())).collect()
}

/// A delivery.
pub fn delivery(d: delivery::Reader<'_>) -> Result<Record, RunError> {
    let mut v = Values::new();
    put(&mut v, "group", id_text(d.get_group())?);
    put(&mut v, "recipient", text(d.get_recipient())?);
    put_all(&mut v, "rights", rights_of(d.get_rights())?);
    put(&mut v, "passOn", yes_no(d.get_pass_on()));
    put_all(&mut v, "via", ids(d.get_via())?);
    put(
        &mut v,
        "state",
        match d.get_state()? {
            DeliveryState::Active => "active",
            DeliveryState::Revoked => "revoked",
        },
    );
    put(&mut v, "origin", text(d.get_origin())?);
    Ok(Record {
        id: id_text(d.get_id())?,
        values: v,
    })
}

/// A source.
pub fn source(s: source::Reader<'_>) -> Result<Record, RunError> {
    let offer = s.get_offer()?;
    let mut v = Values::new();
    put(&mut v, "group", id_text(s.get_group())?);
    put(&mut v, "source", text(offer.get_source())?);
    put(&mut v, "recipient", text(offer.get_recipient())?);
    put_all(&mut v, "rights", rights_of(offer.get_rights())?);
    put(&mut v, "passOn", yes_no(offer.get_pass_on()));
    put(&mut v, "origin", text(offer.get_origin())?);
    put(&mut v, "delivery", id_text(offer.get_delivery())?);
    put(&mut v, "sender", text(s.get_sender())?);
    put(
        &mut v,
        "state",
        match s.get_state()? {
            SourceState::Active => "active",
            SourceState::Withdrawn => "withdrawn",
        },
    );
    Ok(Record {
        id: id_text(s.get_id())?,
        values: v,
    })
}
