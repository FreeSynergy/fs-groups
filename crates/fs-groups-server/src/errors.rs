//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "a cycle" from "a duplicate" from "not possible" without reading a message. A failure on
//! data already stored is not the client's fault, so it is never a 4xx.

use fs_groups_core::Error;
use fs_interfaces::rpc::{Problem, failed, invalid_request, unsupported};

const NOT_FOUND: &str = "https://freesynergy.net/problems/groups/not-found";
const CYCLE: &str = "https://freesynergy.net/problems/groups/cycle";
const CONFLICT: &str = "https://freesynergy.net/problems/groups/conflict";
const INVALID_TRANSITION: &str = "https://freesynergy.net/problems/groups/invalid-transition";
const NOT_INVITEE: &str = "https://freesynergy.net/problems/groups/not-invitee";
const FORBIDDEN: &str = "https://freesynergy.net/problems/groups/forbidden";
const UNMAPPED: &str = "https://freesynergy.net/problems/groups/unmapped";
const EXPIRED: &str = "https://freesynergy.net/problems/groups/expired";
const DIRECTORY: &str = "https://freesynergy.net/problems/groups/directory";

/// A core error as an RPC error.
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    let text = e.to_string();
    let problem =
        |kind, title, status| Problem::new(kind, title, text.clone(), status).into_error();
    match e {
        Error::Unsupported(not_possible) => unsupported(&not_possible),
        Error::NotFound { .. } => problem(NOT_FOUND, "Not found", 404),
        Error::Cycle { .. } => problem(CYCLE, "Nesting cycle", 409),
        Error::Conflict(_) => problem(CONFLICT, "Conflict", 409),
        Error::InvalidTransition { .. } => {
            problem(INVALID_TRANSITION, "Invitation already answered", 409)
        }
        Error::NotInvitee(_) => problem(NOT_INVITEE, "Not the invitee", 403),
        Error::Forbidden(_) => problem(FORBIDDEN, "Forbidden", 403),
        Error::Unmapped(_) => problem(UNMAPPED, "People not mapped", 502),
        Error::Expired(_) => problem(EXPIRED, "Invitation expired", 410),
        Error::Invalid(_) => invalid_request(text),
        Error::Directory(_) => problem(DIRECTORY, "Directory failed", 502),
        Error::Store(store) => store_error(&store),
        Error::Json(_) => failed(text),
    }
}

/// What the store refuses is said by name; what it cannot do (backend, key, stored JSON) is a failure
/// of this program.
fn store_error(e: &fs_objects::Error) -> capnp::Error {
    let text = e.to_string();
    match e {
        fs_objects::Error::NotFound(_) => {
            Problem::new(NOT_FOUND, "Not found", text, 404).into_error()
        }
        fs_objects::Error::Invalid(_) => invalid_request(text),
        fs_objects::Error::Conflict(_) | fs_objects::Error::OriginalImmutable(_) => {
            Problem::new(CONFLICT, "Conflict", text, 409).into_error()
        }
        fs_objects::Error::EncryptionRequired(_)
        | fs_objects::Error::Key(_)
        | fs_objects::Error::Serde(_)
        | fs_objects::Error::Backend(_) => failed(text),
    }
}
