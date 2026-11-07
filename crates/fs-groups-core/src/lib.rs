//! `fs-groups-core`: program core of FreeSynergy groups (no GUI; the Cap'n Proto server is `fs-groups-server`).
//!
//! A group is an AS2 `Group` in an envelope stored through `fs-objects` (ADR 0014, 0020, 0050). Groups
//! nest (several parents allowed, a cycle is refused), people belong to them with a role, people are
//! invited, and rights are delegated more finely than all-or-nothing. It runs alone (ADR 0024); a
//! directory such as Kanidm is only a mirror behind [`GroupDirectory`] (ADR 0047).
//! Start with [`Groups`] and [`GroupRepo`]. See `DESIGN.md` for the pattern map.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// Pedantic lints that would only add noise to this crate's small, documented API: every fallible fn
// returns the crate `Result` (documented once on `Error`), and results of pure getters are used anyway.
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions
)]

mod error;
pub mod graph;
mod invitation_state;
pub mod mirror;
pub mod model;
mod repo;
pub mod right_flow;
pub mod traits;
mod unsupported;

pub use error::{Error, Result};
pub use graph::Graph;
pub use invitation_state::Step;
pub use mirror::{Change, Divergence, Mirror, SyncReport, plan};
pub use model::{
    Cursor, Delegation, EffectiveMember, Group, Invitation, InvitationState, Member, Membership,
    Page, ParentRequest, RequestState, Right, RightFlow,
};
pub use repo::{
    CONTEXT, DELEGATION_TYPE, GROUP_TYPE, GroupRepo, INVITATION_TYPE, MEMBERSHIP_TYPE,
    PARENT_REQUEST_TYPE,
};
pub use traits::{
    Delegations, DirectoryGroup, GroupChange, GroupDirectory, GroupFeed, GroupListener, Groups,
    Invitations, Memberships, Registry, Subscription,
};
pub use unsupported::not_possible;
