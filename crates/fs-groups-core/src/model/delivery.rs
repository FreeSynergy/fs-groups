//! A delivery: rights a group lets another, independent group have (provisional, ADR 0056).
//!
//! Pattern: State (small) + Value object — a delivery is `Active` until the giver revokes it, and
//! never becomes active again. Following is not nesting: a delivery has no parent and no tree.

use fs_objects::ObjectId;

use super::Right;

/// Where a delivery stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeliveryState {
    /// The giver lets the receiver have the rights.
    Active,
    /// The giver (or the loss of the source it relayed) ended it.
    Revoked,
}

impl DeliveryState {
    /// Stable text used in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }

    /// Inverse of [`DeliveryState::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Active, Self::Revoked]
            .into_iter()
            .find(|x| x.as_str() == s)
    }
}

/// What the giver delivers to one receiving group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    /// Id of the delivery (the receiver's offer names it).
    pub id: ObjectId,
    /// The giving group on this server (for a relayed delivery: the group that holds the source).
    pub group: ObjectId,
    /// IRI of the receiving group, `https://<instance>/groups/<id>`.
    pub recipient: String,
    /// The rights given; never `Manage` (provisional).
    pub rights: Vec<Right>,
    /// Whether the receiver may pass it on to a third group. Not carried by a relayed copy.
    pub pass_on: bool,
    /// The source on this server this delivery relays; `None` for the group's own rights.
    pub via: Option<ObjectId>,
    /// Current state.
    pub state: DeliveryState,
    /// IRI of the group whose right this is: the giving group, or for a relayed copy the first source.
    pub origin: String,
}

/// What reaches the receiver: enough to enter the source (the content of the AS2 `Invite`, proposal).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// Id of the delivery at the giver.
    pub delivery: ObjectId,
    /// IRI of the giving group.
    pub source: String,
    /// IRI of the receiving group.
    pub recipient: String,
    /// The rights offered.
    pub rights: Vec<Right>,
    /// Whether the receiver may pass it on.
    pub pass_on: bool,
    /// IRI of the group whose right this is; equals `source` unless the giver relays.
    pub origin: String,
}

impl Offer {
    /// The offer as the AS2 `Invite` that travels to the receiver (proposal).
    pub fn as2(&self) -> serde_json::Value {
        serde_json::json!({
            "@context": ["https://www.w3.org/ns/activitystreams", {"fs": super::NS}],
            "type": "Invite", "actor": self.source, "object": self.source, "target": self.recipient,
            "fs:delivery": self.delivery.to_string(), "fs:origin": self.origin, "fs:passOn": self.pass_on,
            "fs:rights": self.rights.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
        })
    }
}
