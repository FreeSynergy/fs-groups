//! Value types of the groups contract.
//!
//! Pattern: Value objects — plain data, one file per concept; no storage logic.

mod delegation;
mod group;
mod invitation;
mod member;
mod page;

pub use delegation::{Delegation, Right};
pub use group::{Group, NS};
pub use invitation::{Invitation, InvitationState};
pub use member::{EffectiveMember, Member, Membership};
pub use page::{Cursor, Page};
