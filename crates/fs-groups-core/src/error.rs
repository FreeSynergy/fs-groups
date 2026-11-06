//! Error type of the groups core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation, with
//! `From` conversions for the store and JSON layers. Cycles and bad invitation steps are named errors.

use fs_store::ObjectId;

use crate::model::InvitationState;

/// Result of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

fn path_text(path: &[ObjectId]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Everything that can go wrong in the groups core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The object store failed.
    #[error("object store: {0}")]
    Store(#[from] fs_store::Error),
    /// Stored JSON could not be read or written.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// No object of this kind with this id.
    #[error("{what} {id} not found")]
    NotFound {
        /// What was looked for (`group`, `membership`, `invitation`, `delegation`).
        what: &'static str,
        /// The id that was not found.
        id: ObjectId,
    },
    /// Nesting would make a group its own ancestor. `path` runs from `parent` up to `group`.
    #[error(
        "refused: {parent} cannot be a parent of {group}, since {group} is already an ancestor of {parent} (path {})",
        path_text(.path)
    )]
    Cycle {
        /// The group that was to get a parent.
        group: ObjectId,
        /// The proposed parent.
        parent: ObjectId,
        /// Parent chain from `parent` up to `group` (just `group` for a self-parent).
        path: Vec<ObjectId>,
    },
    /// The operation conflicts with the current data (duplicate, wrong kind of member, ...).
    #[error("{0}")]
    Conflict(String),
    /// The invitation is not in a state that allows this step.
    #[error("invitation is {from:?} and cannot be {step}")]
    InvalidTransition {
        /// State the invitation is in.
        from: InvitationState,
        /// The step that was tried (`accepted`, `declined`, ...).
        step: &'static str,
    },
    /// The person who tried to answer is not the one invited.
    #[error("invitation {0} was not sent to this person")]
    NotInvitee(ObjectId),
    /// The acting person may not do this (see the notes on what is and is not enforced in `DESIGN.md`).
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// The directory applied the group but could not map these people (they are left out).
    #[error("directory could not map these people: {}", .0.join(", "))]
    Unmapped(Vec<String>),
    /// The invitation ran out before it was answered (it is now stored as `expired`).
    #[error("invitation {0} has expired")]
    Expired(ObjectId),
    /// A directory (mirror target) failed; the text comes from the adapter.
    #[error("directory: {0}")]
    Directory(String),
    /// The explicit "not possible" of a provider (ADR 0045).
    #[error(transparent)]
    Unsupported(#[from] fs_interfaces::Unsupported),
}
