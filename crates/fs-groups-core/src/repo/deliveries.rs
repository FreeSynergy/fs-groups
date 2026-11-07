//! Deliveries: the giver's side of following (provisional, ADR 0056).
//!
//! Pattern: Repository + State + Specification — a delivery is stored per receiving group and is
//! `Active` until revoked; the rules (`follow_rules`) decide what may be given. The giver can give only
//! what the acting person holds there (`may_do`) and never `Manage`; a relayed delivery only within a
//! source that allows passing on. **Not enforced:** `actor` is a string nobody authenticates, as for
//! nesting (`DESIGN.md`); `pass_on` does not ask whether the actor may act for the receiving group.

use fs_store::{ObjectId, Order, Query, StoredObject};
use serde_json::json;

use super::consts::{AS2_TYPE, CONTEXT, DELIVERY_TYPE};
use super::envelope::context;
use super::follow_codec::{delivery, offer_fields};
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::follow_rules::{may_pass_on, normalise, within};
use crate::model::{Delivery, DeliveryState, Offer, Right};

impl GroupRepo {
    /// `follow.deliver`: let the group `recipient` have `rights` on `group`. Refused with
    /// [`Error::Forbidden`] when the acting person does not hold one of them on `group`, or for
    /// `Manage`. An unknown group is [`Error::NotFound`]. Asking again for the same thing returns
    /// the active delivery that exists.
    pub async fn deliver(
        &self,
        group: ObjectId,
        recipient: &str,
        rights: &[Right],
        pass_on: bool,
    ) -> Result<(Delivery, Offer)> {
        let rights = normalise(rights)?;
        let w = self.writer().await;
        w.require(group).await?;
        for r in &rights {
            if !w.may_do(group, &self.actor, *r).await? {
                return Err(Error::Forbidden(format!(
                    "{} does not hold {} on group {group}, so cannot give it",
                    self.actor,
                    r.as_str()
                )));
            }
        }
        w.store_delivery(group, recipient, rights, pass_on, None)
            .await
    }

    /// `follow.deliver`: pass the received source `source` on to the group `recipient` (IRI).
    /// Only if it is active and its giver allowed passing on, and only within the rights it gave.
    /// The copy cannot be passed on again.
    pub async fn pass_on(
        &self,
        source: ObjectId,
        recipient: &str,
        rights: &[Right],
    ) -> Result<(Delivery, Offer)> {
        let rights = normalise(rights)?;
        let w = self.writer().await;
        let s = w.source(source).await?;
        if !may_pass_on(&s) || !within(&s.offer.rights, &rights) {
            return Err(Error::Forbidden(format!(
                "source {source} was not given to be passed on, or not with these rights"
            )));
        }
        if recipient.trim() == s.offer.source {
            return Err(Error::Invalid(
                "a source is not passed back to itself".into(),
            ));
        }
        w.store_delivery(s.group, recipient, rights, false, Some(source))
            .await
    }

    /// `follow.deliver`: the giver ends a delivery, at any time. Ending it twice changes nothing.
    pub async fn revoke_delivery(&self, delivery_id: ObjectId) -> Result<()> {
        let w = self.writer().await;
        let o = w
            .load(delivery_id)
            .await?
            .filter(|o| delivery(o).is_some())
            .ok_or(Error::NotFound {
                what: "delivery",
                id: delivery_id,
            })?;
        w.set_text_state(o, DeliveryState::Revoked.as_str()).await
    }

    /// `follow.read`: the deliveries `group` makes, in any state, oldest first.
    pub async fn deliveries(&self, group: ObjectId) -> Result<Vec<Delivery>> {
        self.require(group).await?;
        self.deliveries_where(&[("group", group.to_string())]).await
    }

    pub(super) async fn deliveries_where(&self, keys: &[(&str, String)]) -> Result<Vec<Delivery>> {
        let mut q = Query::new()
            .of_type(DELIVERY_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::PublishedAsc);
        for (k, v) in keys {
            q = q.with_index(*k, v.clone());
        }
        Ok(self
            .reads
            .query(&q)
            .await?
            .iter()
            .filter_map(delivery)
            .collect())
    }
}

impl Writer<'_> {
    async fn store_delivery(
        &self,
        group: ObjectId,
        recipient: &str,
        rights: Vec<Right>,
        pass_on: bool,
        via: Option<ObjectId>,
    ) -> Result<(Delivery, Offer)> {
        let recipient = recipient.trim();
        if !recipient.starts_with("https://") || recipient == self.iri(group) {
            return Err(Error::Invalid(format!(
                "{recipient:?} is not another group's IRI"
            )));
        }
        let make = |id: ObjectId, state| {
            let offer = Offer {
                delivery: id,
                source: self.iri(group),
                recipient: recipient.into(),
                rights: rights.clone(),
                pass_on,
            };
            let d = Delivery {
                id,
                group,
                recipient: recipient.into(),
                rights: rights.clone(),
                pass_on,
                via,
                state,
            };
            (d, offer)
        };
        let mut same = vec![
            ("group", group.to_string()),
            ("recipient", recipient.into()),
        ];
        same.push(("state", DeliveryState::Active.as_str().into()));
        if let Some(d) = self
            .deliveries_where(&same)
            .await?
            .into_iter()
            .find(|d| d.rights == rights && d.pass_on == pass_on && d.via == via)
        {
            return Ok(make(d.id, d.state));
        }
        let env = self.fresh_envelope(DELIVERY_TYPE, &format!("{group} delivers to {recipient}"));
        let (d, offer) = make(env.id, DeliveryState::Active);
        let mut obj = offer_fields(
            StoredObject::new(
                env,
                AS2_TYPE,
                json!({"@context": context(), "type": DELIVERY_TYPE, "object": offer.source, "target": recipient}),
            ),
            group,
            &offer,
            DeliveryState::Active.as_str(),
        );
        if let Some(v) = via {
            obj = obj.with_index("via", v.to_string());
        }
        self.put(obj).await?;
        Ok((d, offer))
    }

    /// Set the `state` of a stored delivery or source (index and content).
    pub(super) async fn set_text_state(&self, mut o: StoredObject, state: &str) -> Result<()> {
        if o.index
            .get("state")
            .and_then(|v| v.first())
            .map(String::as_str)
            == Some(state)
        {
            return Ok(()); // already there: nothing is written, nothing is told
        }
        o.index.insert("state".into(), vec![state.into()]);
        o.content["fs:state"] = json!(state);
        self.put(o).await
    }
}
