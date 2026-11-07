//! The store-backed groups program: module map.
//!
//! Pattern: Facade (declarations) — `GroupRepo` is the one entry point; each file is one concept
//! (items, nesting, memberships, effective membership, invitations, delegations, mirror ledger).

mod backend;
mod codec;
mod consts;
mod delegations;
mod deliveries;
mod effective;
mod envelope;
mod follow_codec;
mod handle;
mod invitations;
mod items;
mod members;
mod mirror;
mod nesting;
mod notify;
mod parent_requests;
mod provide;
mod sources;
mod support;

pub use consts::{
    CONTEXT, DELEGATION_TYPE, DELIVERY_TYPE, GROUP_TYPE, INVITATION_TYPE, MEMBERSHIP_TYPE,
    PARENT_REQUEST_TYPE, SOURCE_TYPE,
};
pub use handle::GroupRepo;
