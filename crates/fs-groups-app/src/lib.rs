//! `fs-groups-app`: the window of the groups program (ADR 0005, 0009, 0063).
//!
//! The groups program talks over `groups.capnp`; this crate is the frontend: a proxy to the server
//! ([`GroupsRemote`]), an adapter to the list-and-page workbench of `fs-components`
//! ([`GroupsAdapter`]) and the window ([`gui::run`]). It imports no `blinc_*` crate: Blinc comes only through `fs-components`.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

mod adapter;
pub mod gui;
mod remote;
mod words;

pub use adapter::GroupsAdapter;
pub use remote::{ApiError, Group, GroupsApi, GroupsRemote, Limits, Member};
pub use words::{TITLE, labels};
