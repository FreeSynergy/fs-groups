//! `fs-groups-app`: the window of the groups program (ADR 0005, 0009, 0063, 0065, 0066).
//!
//! The groups program talks over `groups.capnp`; this crate is **shell and wiring only**: it reads the catalog the
//! server serves (`Groups.types`), gives each data type a [`GroupsBackend`] over the socket and lets
//! `fs-admin-frontend` generate list, detail, forms and command buttons inside the standard window shell
//! (`fs-components-app`). It has no render code and no look of its own (ADR 0066); it imports no `blinc_*` crate.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

mod backend;
pub mod gui;
mod remote;
mod words;

pub use backend::{GroupChange, GroupsBackend, plan_change};
pub use remote::{Clients, GroupsRemote, RunError};
pub use words::{English, TITLE, humanise};
