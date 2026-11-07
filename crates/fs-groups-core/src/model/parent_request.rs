//! A request to nest a group under a parent, which the parent has to approve (provisional).
//!
//! Pattern: State (small) + Value object — the three states are an enum; the repository moves a
//! request only from `Pending`.

use fs_objects::ObjectId;

/// Where a request to become a child stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequestState {
    /// Waiting for the parent's answer.
    Pending,
    /// Approved on behalf of the parent: the link exists.
    Approved,
    /// Declined on behalf of the parent.
    Declined,
}

impl RequestState {
    /// Stable text used in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Declined => "declined",
        }
    }

    /// Inverse of [`RequestState::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Pending, Self::Approved, Self::Declined]
            .into_iter()
            .find(|x| x.as_str() == s)
    }
}

/// A group asks to become a child of another. The link exists only once the request is approved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentRequest {
    /// Id of the request.
    pub id: ObjectId,
    /// The group that wants a parent.
    pub child: ObjectId,
    /// The group that would become its parent; its side decides.
    pub parent: ObjectId,
    /// Who asked (the acting person of the repository at that time).
    pub requester: String,
    /// Current state.
    pub state: RequestState,
}
