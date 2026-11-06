//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "a cycle" from "a duplicate" from "not possible" without reading a message.

use fs_groups_core::Error;
use fs_interfaces::rpc::{Problem, failed, unsupported};

const NOT_FOUND: &str = "https://freesynergy.net/problems/groups/not-found";
const CYCLE: &str = "https://freesynergy.net/problems/groups/cycle";
const CONFLICT: &str = "https://freesynergy.net/problems/groups/conflict";
const INVALID_TRANSITION: &str = "https://freesynergy.net/problems/groups/invalid-transition";
const NOT_INVITEE: &str = "https://freesynergy.net/problems/groups/not-invitee";
const FORBIDDEN: &str = "https://freesynergy.net/problems/groups/forbidden";
const UNMAPPED: &str = "https://freesynergy.net/problems/groups/unmapped";
const EXPIRED: &str = "https://freesynergy.net/problems/groups/expired";
const INVALID: &str = "https://freesynergy.net/problems/groups/invalid";
const DIRECTORY: &str = "https://freesynergy.net/problems/groups/directory";
const TOO_LARGE: &str = "https://freesynergy.net/problems/groups/too-large";

/// A core error as an RPC error.
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    let text = e.to_string();
    match e {
        Error::Unsupported(not_possible) => unsupported(&not_possible),
        Error::NotFound { .. } => Problem::new(NOT_FOUND, "Not found", text, 404).into_error(),
        Error::Cycle { .. } => Problem::new(CYCLE, "Nesting cycle", text, 409).into_error(),
        Error::Conflict(_) => Problem::new(CONFLICT, "Conflict", text, 409).into_error(),
        Error::InvalidTransition { .. } => {
            Problem::new(INVALID_TRANSITION, "Invitation already answered", text, 409).into_error()
        }
        Error::Expired(_) => Problem::new(EXPIRED, "Invitation expired", text, 410).into_error(),
        Error::NotInvitee(_) => Problem::new(NOT_INVITEE, "Not the invitee", text, 403).into_error(),
        Error::Forbidden(_) => Problem::new(FORBIDDEN, "Forbidden", text, 403).into_error(),
        Error::Unmapped(_) => Problem::new(UNMAPPED, "People not mapped", text, 502).into_error(),
        Error::Invalid(_) => invalid(text),
        Error::Directory(_) => Problem::new(DIRECTORY, "Directory failed", text, 502).into_error(),
        Error::Store(_) | Error::Json(_) => failed(text),
    }
}

/// The request itself is not acceptable.
pub(crate) fn invalid(detail: impl Into<String>) -> capnp::Error {
    Problem::new(INVALID, "Invalid request", detail, 422).into_error()
}

/// An answer in a list without a cursor would not fit one message (`fs_interfaces::rpc`).
pub(crate) fn too_large(detail: impl Into<String>) -> capnp::Error {
    Problem::new(TOO_LARGE, "Answer too large", detail, 413).into_error()
}
