//! Invitations and their states.
//!
//! Pattern: State — the states are an enum; the one transition function is in `invitation_state.rs`.

use fs_objects::{ObjectId, Timestamp};

/// Where an invitation stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InvitationState {
    /// Sent, not answered.
    Pending,
    /// Accepted; a membership exists.
    Accepted,
    /// Declined by the invitee.
    Declined,
    /// Withdrawn by the group.
    Revoked,
    /// Ran out unanswered.
    Expired,
}

impl InvitationState {
    /// Stable text used in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    /// Inverse of [`InvitationState::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Accepted,
            Self::Declined,
            Self::Revoked,
            Self::Expired,
        ]
        .into_iter()
        .find(|x| x.as_str() == s)
    }
}

/// An invitation to join a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitation {
    /// Id of the invitation.
    pub id: ObjectId,
    /// The group.
    pub group: ObjectId,
    /// IRI or e-mail address of the invitee.
    pub invitee: String,
    /// Role offered; empty = plain member.
    pub role: String,
    /// Current state.
    pub state: InvitationState,
    /// After this moment a pending invitation cannot be accepted; `None` = never.
    pub expires: Option<Timestamp>,
}

impl Invitation {
    /// Whether `person` may answer this invitation. `person` is the invitee exactly (IRIs are case
    /// sensitive). `proved` is an address the caller vouches the person controls: it counts only for
    /// an invitee that is an e-mail address, compared ignoring case. **Provisional** (see
    /// `GroupRepo::accept_proved`).
    pub fn is_for(&self, person: &str, proved: Option<&str>) -> bool {
        if self.invitee == person {
            return true;
        }
        let is_mail = self.invitee.contains('@') && !self.invitee.contains("://");
        is_mail && proved.is_some_and(|p| p.trim().eq_ignore_ascii_case(&self.invitee))
    }

    /// Whether the expiry time has passed at `now`. The moment itself counts as passed.
    pub fn ran_out(&self, now: Timestamp) -> bool {
        self.expires.is_some_and(|e| e <= now)
    }
}
