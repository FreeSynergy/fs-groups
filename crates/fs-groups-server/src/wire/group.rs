//! A group and a page of groups on the wire.
//!
//! Pattern: Adapter — `Group` between Rust and `groups.capnp`. The AS2 JSON stays authoritative for the
//! content; the envelope is the routing summary.

use fs_groups_core::{Group, RightFlow};
use fs_interfaces::rpc::{DEFAULT_PAGE_SIZE, Page, write_envelope, write_object_id};
use fs_interfaces::{common_capnp, groups_capnp};

use super::basic::list_len;
use crate::errors::{invalid, too_large};

/// Fixed space of an envelope and a group on the wire, on top of their text.
const GROUP_BYTES: usize = 512;
const ID_BYTES: usize = 16;

fn flow_to_wire(flow: RightFlow) -> groups_capnp::RightFlow {
    match flow {
        RightFlow::None => groups_capnp::RightFlow::None,
        RightFlow::Down => groups_capnp::RightFlow::Down,
        RightFlow::Up => groups_capnp::RightFlow::Up,
    }
}

/// The right flow of a request.
pub(crate) fn read_flow(
    flow: Result<groups_capnp::RightFlow, capnp::NotInSchema>,
) -> capnp::Result<RightFlow> {
    Ok(match flow.map_err(|e| invalid(e.to_string()))? {
        groups_capnp::RightFlow::None => RightFlow::None,
        groups_capnp::RightFlow::Down => RightFlow::Down,
        groups_capnp::RightFlow::Up => RightFlow::Up,
    })
}

/// Write one group.
pub(crate) fn write_group(
    mut out: groups_capnp::group::Builder<'_>,
    group: &Group,
) -> capnp::Result<()> {
    write_envelope(out.reborrow().init_envelope(), &group.envelope);
    out.set_as2(group.as2().to_string().as_str());
    out.set_kind(group.kind.as_str());
    out.set_right_flow(flow_to_wire(group.right_flow));
    let mut parents = out.init_parents(list_len(group.parents.len(), weight(group))?);
    for (i, id) in (0u32..).zip(&group.parents) {
        write_object_id(parents.reborrow().get(i), *id);
    }
    Ok(())
}

/// About what the group takes on the wire: the envelope's texts, the AS2 text, kind and parents.
fn weight(group: &Group) -> usize {
    let e = &group.envelope;
    let lists = [&e.to, &e.cc, &e.bto, &e.bcc, &e.audience, &e.sensitive];
    let listed: usize = lists.iter().flat_map(|l| l.iter()).map(String::len).sum();
    let optional = [&e.iri, &e.attributed_to, &e.context, &e.in_reply_to, &e.describes];
    let single: usize = optional.iter().filter_map(|t| t.as_ref()).map(String::len).sum();
    let as2 = group.name.len() + group.kind.len() + group.iri.len() + group.parents.len() * 40;
    GROUP_BYTES + listed + single + e.name.len() + as2 + group.kind.len() + group.parents.len() * ID_BYTES
}

/// Cut the page at `cursor` and write it.
pub(crate) fn write_page(
    mut out: groups_capnp::group_page::Builder<'_>,
    all: Vec<Group>,
    cursor: capnp::Result<common_capnp::cursor::Reader<'_>>,
) -> capnp::Result<()> {
    let opaque = cursor?.get_opaque()?;
    let page = Page::of_weighted(all, opaque, DEFAULT_PAGE_SIZE, weight)
        .map_err(|e| invalid(e.extra))?;
    let count = u32::try_from(page.items.len()).map_err(|_| too_large("page too long"))?;
    let mut items = out.reborrow().init_items(count);
    for (i, group) in (0u32..).zip(&page.items) {
        write_group(items.reborrow().get(i), group)?;
    }
    out.init_next().set_opaque(&page.next);
    Ok(())
}
