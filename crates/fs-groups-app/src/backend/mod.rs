//! The `Backend` of the generated Admin frontend over the groups server.
//!
//! Pattern: Repository (the frontend's `Backend` trait, trait first) implemented as a Facade over the Command
//! functions of `groups_ops`, `members_ops` and `follow_ops` — one `GroupsBackend` per data type, the table of
//! commands in the core (`fs_groups_core::describe`) decides what exists, what is supported and what a command
//! covers. No rule of groups lives here.
//!
//! A command the table marks as not supported is refused **before** any call: `run_typed` answers
//! `RunError::Unsupported`, the `Backend` trait (which has no such variant) answers `NotFound`, "no such command".

mod delegations_ops;
mod facade;
mod follow_ops;
mod groups_ops;
mod input;
mod invitations_ops;
mod members_ops;
mod read;

pub use facade::GroupsBackend;
pub use groups_ops::{GroupChange, plan_change};
