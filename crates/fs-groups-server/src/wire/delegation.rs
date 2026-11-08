//! Delegations and rights on the wire.
//!
//! Pattern: Adapter — `Delegation` and `Right` between Rust and `groups.capnp`.

use fs_groups_core::{Delegation, Right};
use fs_interfaces::groups_capnp::{self, delegation};
use fs_interfaces::rpc::{invalid_request, write_object_id};

use super::basic::{ENTRY_BYTES, list_len};
use super::member::write_member;

pub(super) fn right_to_wire(right: Right) -> groups_capnp::Right {
    match right {
        Right::Read => groups_capnp::Right::Read,
        Right::AddMembers => groups_capnp::Right::AddMembers,
        Right::RemoveMembers => groups_capnp::Right::RemoveMembers,
        Right::Manage => groups_capnp::Right::Manage,
    }
}

/// The right of a request.
pub(crate) fn read_right(
    right: Result<groups_capnp::Right, capnp::NotInSchema>,
) -> capnp::Result<Right> {
    Ok(match right.map_err(|e| invalid_request(e.to_string()))? {
        groups_capnp::Right::Read => Right::Read,
        groups_capnp::Right::AddMembers => Right::AddMembers,
        groups_capnp::Right::RemoveMembers => Right::RemoveMembers,
        groups_capnp::Right::Manage => Right::Manage,
    })
}

/// Write one delegation.
pub(crate) fn write_delegation(mut out: delegation::Builder<'_>, d: &Delegation) {
    write_object_id(out.reborrow().init_id(), d.id);
    write_object_id(out.reborrow().init_group(), d.group);
    write_member(out.reborrow().init_holder(), &d.holder);
    out.set_right(right_to_wire(d.right));
}

/// Write the delegations of a group; `init` makes the list once its length is known to fit.
pub(crate) fn write_delegations<'a>(
    items: &[Delegation],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, delegation::Owned>,
) -> capnp::Result<()> {
    let bytes = items
        .iter()
        .map(|d| ENTRY_BYTES + d.holder.key().len())
        .sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, d) in (0u32..).zip(items) {
        write_delegation(out.reborrow().get(i), d);
    }
    Ok(())
}
