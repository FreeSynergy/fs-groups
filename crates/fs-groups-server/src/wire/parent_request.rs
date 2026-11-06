//! Parent requests on the wire.
//!
//! Pattern: Adapter — `ParentRequest` between Rust and `groups.capnp`. `requester` is the string the
//! provider acts under; it says who asked as far as anyone claimed, and proves nothing.

use fs_groups_core::{ParentRequest, RequestState};
use fs_interfaces::groups_capnp::{self, parent_request};
use fs_interfaces::rpc::write_object_id;

use super::basic::{ENTRY_BYTES, list_len};

fn state_to_wire(state: RequestState) -> groups_capnp::RequestState {
    match state {
        RequestState::Pending => groups_capnp::RequestState::Pending,
        RequestState::Approved => groups_capnp::RequestState::Approved,
        RequestState::Declined => groups_capnp::RequestState::Declined,
    }
}

/// Write one parent request.
pub(crate) fn write_parent_request(mut out: parent_request::Builder<'_>, r: &ParentRequest) {
    write_object_id(out.reborrow().init_id(), r.id);
    write_object_id(out.reborrow().init_child(), r.child);
    write_object_id(out.reborrow().init_parent(), r.parent);
    out.set_requester(r.requester.as_str());
    out.set_state(state_to_wire(r.state));
}

/// Write the requests to a group; `init` makes the list once its length is known to fit.
pub(crate) fn write_parent_requests<'a>(
    items: &[ParentRequest],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, parent_request::Owned>,
) -> capnp::Result<()> {
    let bytes = items.iter().map(|r| ENTRY_BYTES + r.requester.len()).sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, r) in (0u32..).zip(items) {
        write_parent_request(out.reborrow().get(i), r);
    }
    Ok(())
}
