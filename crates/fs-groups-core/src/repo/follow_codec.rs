//! Stored object -> typed value for deliveries and sources, and the index fields they are written with.
//!
//! Pattern: Adapter — like `codec.rs`: the values are read from the index fields written next to the
//! AS2 content; a malformed object reads as `None`.

use fs_store::{ObjectId, StoredObject};

use super::codec::{id_of, one};
use super::consts::{DELIVERY_TYPE, SOURCE_TYPE};
use crate::model::{Delivery, DeliveryState, Offer, Right, Source, SourceState};

fn rights(o: &StoredObject) -> Option<Vec<Right>> {
    o.index
        .get("rights")?
        .iter()
        .map(|r| Right::parse(r))
        .collect()
}

/// The index fields of an offer, shared by deliveries and sources.
pub(super) fn offer_fields(
    o: StoredObject,
    group: ObjectId,
    offer: &Offer,
    state: &str,
) -> StoredObject {
    let mut o = o
        .with_index("group", group.to_string())
        .with_index("delivery", offer.delivery.to_string())
        .with_index("source_iri", offer.source.clone())
        .with_index("recipient", offer.recipient.clone())
        .with_index("pass_on", offer.pass_on.to_string())
        .with_index("state", state);
    for r in &offer.rights {
        o = o.with_index("rights", r.as_str());
    }
    o
}

pub(super) fn offer(o: &StoredObject) -> Option<Offer> {
    Some(Offer {
        delivery: id_of(o, "delivery")?,
        source: one(o, "source_iri")?.to_string(),
        recipient: one(o, "recipient")?.to_string(),
        rights: rights(o)?,
        pass_on: one(o, "pass_on")? == "true",
    })
}

pub(super) fn delivery(o: &StoredObject) -> Option<Delivery> {
    (o.envelope.object_type == DELIVERY_TYPE).then_some(())?;
    let offer = offer(o)?;
    Some(Delivery {
        id: o.envelope.id,
        group: id_of(o, "group")?,
        recipient: offer.recipient,
        rights: offer.rights,
        pass_on: offer.pass_on,
        via: id_of(o, "via"),
        state: DeliveryState::parse(one(o, "state")?)?,
    })
}

pub(super) fn source(o: &StoredObject) -> Option<Source> {
    (o.envelope.object_type == SOURCE_TYPE).then_some(())?;
    Some(Source {
        id: o.envelope.id,
        group: id_of(o, "group")?,
        offer: offer(o)?,
        sender: one(o, "sender")?.to_string(),
        state: SourceState::parse(one(o, "state")?)?,
    })
}
