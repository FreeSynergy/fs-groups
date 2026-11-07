//! A source: what a group has been offered and has entered, without confirmation (provisional).
//!
//! Pattern: State (small) + Value object — a source is `Active` until the giver withdraws it; the
//! receiver may remove it at any time (then the record is gone).

use fs_objects::ObjectId;

use super::Offer;

/// Where a source stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceState {
    /// The giver still delivers.
    Active,
    /// The giver revoked the delivery; the entry stays visible until the receiver removes it.
    Withdrawn,
}

impl SourceState {
    /// Stable text used in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Withdrawn => "withdrawn",
        }
    }

    /// Inverse of [`SourceState::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Active, Self::Withdrawn]
            .into_iter()
            .find(|x| x.as_str() == s)
    }
}

/// A source entered by the receiving group's server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Id of the source on this server.
    pub id: ObjectId,
    /// The receiving group on this server.
    pub group: ObjectId,
    /// What was offered.
    pub offer: Offer,
    /// The origin the transport named when the offer arrived (not authenticated here).
    pub sender: String,
    /// Current state.
    pub state: SourceState,
}
