//! Invitations on the wire.
//!
//! Pattern: Adapter — `Invitation` between Rust and `groups.capnp`. An absent `expires` means never.

use fs_groups_core::{Invitation, InvitationState};
use fs_interfaces::common_capnp::timestamp;
use fs_interfaces::groups_capnp::{self, invitation};
use fs_interfaces::rpc::{from_millis, to_millis, write_object_id};
use fs_store::Timestamp;

use super::basic::{ENTRY_BYTES, list_len, request};

fn state_to_wire(state: InvitationState) -> groups_capnp::InvitationState {
    match state {
        InvitationState::Pending => groups_capnp::InvitationState::Pending,
        InvitationState::Accepted => groups_capnp::InvitationState::Accepted,
        InvitationState::Declined => groups_capnp::InvitationState::Declined,
        InvitationState::Revoked => groups_capnp::InvitationState::Revoked,
        InvitationState::Expired => groups_capnp::InvitationState::Expired,
    }
}

/// The expiry of a request.
pub(crate) fn read_expires(r: capnp::Result<timestamp::Reader<'_>>) -> capnp::Result<Timestamp> {
    from_millis(request(r)?.get_unix_millis())
}

/// Write one invitation.
pub(crate) fn write_invitation(mut out: invitation::Builder<'_>, inv: &Invitation) {
    write_object_id(out.reborrow().init_id(), inv.id);
    write_object_id(out.reborrow().init_group(), inv.group);
    out.set_invitee(inv.invitee.as_str());
    out.set_role(inv.role.as_str());
    out.set_state(state_to_wire(inv.state));
    if let Some(expires) = inv.expires {
        out.init_expires().set_unix_millis(to_millis(expires));
    }
}

/// Write the invitations of a group; `init` makes the list once its length is known to fit.
pub(crate) fn write_invitations<'a>(
    items: &[Invitation],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, invitation::Owned>,
) -> capnp::Result<()> {
    let bytes = items
        .iter()
        .map(|i| ENTRY_BYTES + i.invitee.len() + i.role.len())
        .sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, inv) in (0u32..).zip(items) {
        write_invitation(out.reborrow().get(i), inv);
    }
    Ok(())
}
