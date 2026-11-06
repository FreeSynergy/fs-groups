//! The store-backed groups program: module map.
//!
//! Pattern: Facade (declarations) — `GroupRepo` is the one entry point; each file is one concept
//! (items, nesting, memberships, effective membership, invitations, delegations, mirror ledger).

mod admins;
mod backend;
mod codec;
mod consts;
mod delegations;
mod directory_state;
mod effective;
mod envelope;
mod handle;
mod invitations;
mod items;
mod members;
mod mirror;
mod nesting;
mod parent_requests;
mod provide;
mod support;

pub use consts::{
    CONTEXT, DELEGATION_TYPE, GROUP_TYPE, INVITATION_TYPE, MEMBERSHIP_TYPE, PARENT_REQUEST_TYPE,
};
pub use handle::GroupRepo;
