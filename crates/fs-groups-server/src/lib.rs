//! `fs-groups-server`: the contract `groups.capnp` over a Unix socket, in front of any provider of
//! the groups traits. It only translates: every rule (no cycles, right flow, invitations) stays in the
//! core. `GroupDirectory` is not served here: fs-groups is its client, `fs-identity` its provider.
//!
//! Interface status: draft (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// The generated `Server` traits are async for every method, also those that never wait.
#![allow(clippy::missing_errors_doc, clippy::unused_async_trait_impl)]

mod delegations_service;
mod errors;
mod feed;
mod groups_service;
mod invitations_service;
mod items_service;
mod list_all;
mod memberships_service;
mod nesting_service;
mod operator;
mod wire;

pub use groups_service::GroupsServer;
pub use operator::OperatorIsHuman;
