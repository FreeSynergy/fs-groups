//! Members, memberships and effective members on the wire.
//!
//! Pattern: Adapter — the `Member` union and the two membership shapes of `groups.capnp`.

use fs_groups_core::{EffectiveMember, Member, Membership};
use fs_interfaces::groups_capnp::{effective_member, member, membership};
use fs_interfaces::rpc::{invalid_request, write_object_id};

use super::basic::{ENTRY_BYTES, ID_BYTES, list_len, read_id, read_identifier, request};

/// A member of a request.
pub(crate) fn read_member(r: capnp::Result<member::Reader<'_>>) -> capnp::Result<Member> {
    let r = request(r)?;
    match r.which().map_err(|e| invalid_request(e.to_string()))? {
        member::Which::Person(person) => Ok(Member::Person(read_identifier(person)?.to_string())),
        member::Which::Group(group) => Ok(Member::Group(read_id(group)?)),
    }
}

/// Write a member.
pub(super) fn write_member(mut out: member::Builder<'_>, m: &Member) {
    match m {
        Member::Person(person) => out.set_person(person.as_str()),
        Member::Group(group) => write_object_id(out.init_group(), *group),
    }
}

/// Write one membership.
pub(crate) fn write_membership(mut out: membership::Builder<'_>, m: &Membership) {
    write_object_id(out.reborrow().init_id(), m.id);
    write_object_id(out.reborrow().init_group(), m.group);
    write_member(out.reborrow().init_member(), &m.member);
    out.set_role(m.role.as_str());
}

/// About what one membership takes on the wire.
pub(crate) fn membership_weight(m: &Membership) -> usize {
    ENTRY_BYTES + m.member.key().len() + m.role.len()
}

/// About what one effective member takes on the wire.
pub(crate) fn effective_weight(m: &EffectiveMember) -> usize {
    ENTRY_BYTES + m.person.len() + m.via.len() * ID_BYTES
}

/// Write one page of memberships; `init` makes the list once its length is known to fit.
pub(crate) fn write_memberships<'a>(
    items: &[Membership],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, membership::Owned>,
) -> capnp::Result<()> {
    let bytes = items.iter().map(membership_weight).sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, m) in (0u32..).zip(items) {
        write_membership(out.reborrow().get(i), m);
    }
    Ok(())
}

/// Write one page of effective members; `init` makes the list once its length is known to fit.
pub(crate) fn write_effective<'a>(
    items: &[EffectiveMember],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, effective_member::Owned>,
) -> capnp::Result<()> {
    let bytes = items.iter().map(effective_weight).sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, m) in (0u32..).zip(items) {
        let mut entry = out.reborrow().get(i);
        entry.set_person(m.person.as_str());
        let mut via = entry.init_via(list_len(m.via.len(), m.via.len() * ID_BYTES)?);
        for (j, id) in (0u32..).zip(&m.via) {
            write_object_id(via.reborrow().get(j), *id);
        }
    }
    Ok(())
}
