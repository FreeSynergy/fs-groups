//! Deliveries, offers and sources on the wire.
//!
//! Pattern: Adapter — `Delivery`, `Offer` and `Source` between Rust and `groups.capnp`. Limits on the
//! size of an answer are the same as for every other list.

use fs_groups_core::{Delivery, DeliveryState, Offer, Right, Source, SourceState};
use fs_interfaces::groups_capnp::{self, delivery, offer, source};
use fs_interfaces::rpc::{invalid_request, read_object_id, write_object_id};

use super::basic::{ENTRY_BYTES, list_len};
use super::delegation::{read_right, right_to_wire};

/// The most rights a list can name: there are four, so more is a malformed request.
const MAX_RIGHTS: u32 = 8;

/// The rights of a request.
pub(crate) fn read_rights(
    list: capnp::Result<capnp::enum_list::Reader<'_, groups_capnp::Right>>,
) -> capnp::Result<Vec<Right>> {
    let list = list?;
    if list.len() > MAX_RIGHTS {
        return Err(invalid_request("too many rights"));
    }
    list.iter().map(read_right).collect()
}

fn write_rights(rights: &[Right], out: Option<capnp::enum_list::Builder<'_, groups_capnp::Right>>) {
    if let Some(mut out) = out {
        for (i, r) in (0u32..).zip(rights) {
            out.set(i, right_to_wire(*r));
        }
    }
}

fn rights_len(rights: &[Right]) -> u32 {
    u32::try_from(rights.len()).unwrap_or(MAX_RIGHTS)
}

/// Write one delivery.
pub(crate) fn write_delivery(mut out: delivery::Builder<'_>, d: &Delivery) {
    write_object_id(out.reborrow().init_id(), d.id);
    write_object_id(out.reborrow().init_group(), d.group);
    out.reborrow().set_recipient(&d.recipient);
    write_rights(
        &d.rights,
        Some(out.reborrow().init_rights(rights_len(&d.rights))),
    );
    out.reborrow().set_pass_on(d.pass_on);
    let via: Vec<_> = d.via.into_iter().collect();
    let mut list = out.reborrow().init_via(rights_len_of(via.len()));
    for (i, id) in (0u32..).zip(&via) {
        write_object_id(list.reborrow().get(i), *id);
    }
    out.reborrow().set_state(match d.state {
        DeliveryState::Active => groups_capnp::DeliveryState::Active,
        DeliveryState::Revoked => groups_capnp::DeliveryState::Revoked,
    });
    out.set_origin(&d.origin);
}

fn rights_len_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(1)
}

/// Write one offer.
pub(crate) fn write_offer(mut out: offer::Builder<'_>, o: &Offer) {
    write_object_id(out.reborrow().init_delivery(), o.delivery);
    out.reborrow().set_source(&o.source);
    out.reborrow().set_recipient(&o.recipient);
    write_rights(
        &o.rights,
        Some(out.reborrow().init_rights(rights_len(&o.rights))),
    );
    out.reborrow().set_pass_on(o.pass_on);
    out.reborrow().set_as2(o.as2().to_string());
    out.set_origin(&o.origin);
}

/// The offer of a request. The `as2` text is not read: the core builds it from the fields.
pub(crate) fn read_offer(r: capnp::Result<offer::Reader<'_>>) -> capnp::Result<Offer> {
    let r = r?;
    let text = |t: capnp::Result<capnp::text::Reader<'_>>| -> capnp::Result<String> {
        Ok(t?
            .to_str()
            .map_err(|e| invalid_request(e.to_string()))?
            .to_string())
    };
    Ok(Offer {
        delivery: read_object_id(r.get_delivery()?)?,
        source: text(r.get_source())?,
        recipient: text(r.get_recipient())?,
        rights: read_rights(r.get_rights())?,
        pass_on: r.get_pass_on(),
        origin: text(r.get_origin())?,
    })
}

/// Write one source.
pub(crate) fn write_source(mut out: source::Builder<'_>, s: &Source) {
    write_object_id(out.reborrow().init_id(), s.id);
    write_object_id(out.reborrow().init_group(), s.group);
    write_offer(out.reborrow().init_offer(), &s.offer);
    out.reborrow().set_sender(&s.sender);
    out.set_state(match s.state {
        SourceState::Active => groups_capnp::SourceState::Active,
        SourceState::Withdrawn => groups_capnp::SourceState::Withdrawn,
    });
}

/// Write the deliveries of a group.
pub(crate) fn write_deliveries<'a>(
    items: &[Delivery],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, delivery::Owned>,
) -> capnp::Result<()> {
    let bytes = items
        .iter()
        .map(|d| ENTRY_BYTES * 3 + d.recipient.len() + d.origin.len())
        .sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, d) in (0u32..).zip(items) {
        write_delivery(out.reborrow().get(i), d);
    }
    Ok(())
}

/// Write the sources of a group.
pub(crate) fn write_sources<'a>(
    items: &[Source],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, source::Owned>,
) -> capnp::Result<()> {
    let bytes = items
        .iter()
        .map(|s| {
            ENTRY_BYTES * 4
                + s.sender.len()
                + s.offer.source.len()
                + s.offer.recipient.len()
                + s.offer.origin.len()
        })
        .sum();
    let mut out = init(list_len(items.len(), bytes)?);
    for (i, s) in (0u32..).zip(items) {
        write_source(out.reborrow().get(i), s);
    }
    Ok(())
}
