//! The life of an invitation: one transition function.
//!
//! Pattern: State — `pending` moves to `accepted`, `declined`, `revoked` or `expired`; the others are
//! final. Everything that changes an invitation goes through [`InvitationState::on`], so the rules
//! live in one place. The time is turned into `ran_out` by the caller, so tests need no sleep.

use crate::error::{Error, Result};
use crate::model::InvitationState;

/// What happens to an invitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The invitee accepts.
    Accept,
    /// The invitee declines.
    Decline,
    /// The group withdraws it.
    Revoke,
    /// It is found to have run out.
    Expire,
}

impl Step {
    fn past(self) -> &'static str {
        match self {
            Step::Accept => "accepted",
            Step::Decline => "declined",
            Step::Revoke => "revoked",
            Step::Expire => "expired",
        }
    }
}

impl InvitationState {
    /// The state after `step`, or why not. `ran_out`: the expiry time has passed (see
    /// `Invitation::ran_out`); it only matters for `Accept`, which then ends as `Expired`: the caller
    /// stores that and refuses the answer. `Expire` is for the caller that found the time passed.
    pub fn on(self, step: Step, ran_out: bool) -> Result<InvitationState> {
        use InvitationState::{Accepted, Declined, Expired, Pending, Revoked};
        if self != Pending {
            return Err(Error::InvalidTransition {
                from: self,
                step: step.past(),
            });
        }
        Ok(match step {
            Step::Accept if ran_out => Expired,
            Step::Accept => Accepted,
            Step::Decline => Declined,
            Step::Revoke => Revoked,
            Step::Expire => Expired,
        })
    }
}
