//! Sources: the receiver's side of following (provisional, ADR 0056).
//!
//! Pattern: Repository + State + Specification — a group's sources are what it was offered. The
//! server enters one **without any confirmation**; the receiver can remove it at any time; the
//! giver's withdrawal turns it `Withdrawn`. `follow_rules::from_origin` only checks that the offer's
//! source lies at the origin the transport named: **nothing here authenticates that origin**, the
//! host does. Losing a source (withdrawn or removed) revokes the deliveries relayed from it.

use fs_objects::{ObjectId, Order, Query, StoredObject};

use super::consts::{AS2_TYPE, CONTEXT, SOURCE_TYPE};
use super::follow_codec::{offer_fields, source};
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::follow_rules::{from_origin, local_group, normalise};
use crate::model::{DeliveryState, Offer, Source, SourceState};

impl GroupRepo {
    /// `follow.receive`: enter the offered source for the recipient group, without confirmation. The
    /// source must lie at `sender`, the recipient must be a group of this instance (unknown:
    /// [`Error::NotFound`]). Receiving the same delivery again returns the entry that exists.
    pub async fn receive(&self, sender: &str, offer: &Offer) -> Result<Source> {
        if !from_origin(sender, &offer.source) {
            return Err(Error::Forbidden(format!(
                "{} does not lie at {sender}",
                offer.source
            )));
        }
        if !offer.origin.starts_with("https://") {
            return Err(Error::Invalid(
                "an offer names the group whose right it is".into(),
            ));
        }
        let rights = normalise(&offer.rights)?;
        let group = local_group(&self.instance, &offer.recipient)
            .ok_or_else(|| Error::Invalid(format!("{} is not a group here", offer.recipient)))?;
        let w = self.writer().await;
        w.require(group).await?;
        let known = [
            ("group", group.to_string()),
            ("delivery", offer.delivery.to_string()),
            ("source_iri", offer.source.clone()),
        ];
        if let Some(s) = w.sources_where(&known).await?.into_iter().next() {
            return Ok(s);
        }
        let env = w.fresh_envelope(
            SOURCE_TYPE,
            &format!("{} delivers to {group}", offer.source),
        );
        let id = env.id;
        let offer = Offer {
            rights,
            ..offer.clone()
        };
        let mut content = offer.as2();
        content["fs:sender"] = sender.trim().into();
        let obj = offer_fields(
            StoredObject::new(env, AS2_TYPE, content),
            group,
            &offer,
            "active",
        )
        .with_index("sender", sender.trim());
        w.put(obj).await?;
        Ok(Source {
            id,
            group,
            offer,
            sender: sender.trim().into(),
            state: SourceState::Active,
        })
    }

    /// `follow.receive`: the giver revoked `delivery` of `source` (its IRI): the entry becomes
    /// `Withdrawn` and what was relayed from it is revoked. `sender` must be the origin that sent it.
    pub async fn withdraw(
        &self,
        sender: &str,
        source_iri: &str,
        delivery: ObjectId,
    ) -> Result<Source> {
        if !from_origin(sender, source_iri) {
            return Err(Error::Forbidden(format!(
                "{source_iri} does not lie at {sender}"
            )));
        }
        let w = self.writer().await;
        let keys = [
            ("delivery", delivery.to_string()),
            ("source_iri", source_iri.to_string()),
        ];
        let mut s = w
            .sources_where(&keys)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound {
                what: "source",
                id: delivery,
            })?;
        let o = w.load(s.id).await?.ok_or(Error::NotFound {
            what: "source",
            id: s.id,
        })?;
        w.set_text_state(o, SourceState::Withdrawn.as_str()).await?;
        w.revoke_relays(s.id).await?;
        s.state = SourceState::Withdrawn;
        Ok(s)
    }

    /// `follow.read`: the sources `group` has entered, oldest first. This is how the receiver
    /// sees what it was offered.
    pub async fn sources(&self, group: ObjectId) -> Result<Vec<Source>> {
        self.require(group).await?;
        self.sources_where(&[("group", group.to_string())]).await
    }

    /// `follow.receive`: the receiver removes a source, at any time; what was relayed from it is revoked.
    pub async fn remove_source(&self, id: ObjectId) -> Result<()> {
        let w = self.writer().await;
        w.source(id).await?;
        w.revoke_relays(id).await?;
        w.remove_object(id).await
    }

    async fn sources_where(&self, keys: &[(&str, String)]) -> Result<Vec<Source>> {
        let mut q = Query::new()
            .of_type(SOURCE_TYPE)
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
            .filter_map(source)
            .collect())
    }

    pub(super) async fn source(&self, id: ObjectId) -> Result<Source> {
        let found = self.load(id).await?;
        found
            .as_ref()
            .and_then(source)
            .ok_or(Error::NotFound { what: "source", id })
    }
}

impl Writer<'_> {
    /// The deliveries relayed from source `id` end with it.
    async fn revoke_relays(&self, id: ObjectId) -> Result<()> {
        let relays = self.deliveries_where(&[("via", id.to_string())]).await?;
        for d in relays.iter().filter(|d| d.state == DeliveryState::Active) {
            if let Some(o) = self.load(d.id).await? {
                self.set_text_state(o, DeliveryState::Revoked.as_str())
                    .await?;
            }
        }
        Ok(())
    }
}
