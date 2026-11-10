//! Reading the typed input of a command.
//!
//! Pattern: Adapter — `Values` (texts by field name) to the typed values the wire needs; a value that is not usable
//! is `RunError::Bad` naming its field, so the form can mark it.

use fs_admin_frontend::Values;
use fs_interfaces::groups_capnp::{Right, RightFlow};
use uuid::Uuid;

use crate::remote::RunError;

/// The first value of `field` that is not empty after trimming.
pub fn one<'a>(v: &'a Values, field: &str) -> Option<&'a str> {
    v.get(field)?
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
}

/// All values of `field` that are not empty after trimming.
pub fn many<'a>(v: &'a Values, field: &str) -> Vec<&'a str> {
    v.get(field)
        .map(|l| {
            l.iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn bad(field: &str) -> RunError {
    RunError::Bad {
        field: field.to_owned(),
    }
}

/// The text of a required field.
pub fn need<'a>(v: &'a Values, field: &str) -> Result<&'a str, RunError> {
    one(v, field).ok_or_else(|| bad(field))
}

/// A reference to a group or another record of this program: its id.
pub fn id(v: &Values, field: &str) -> Result<Uuid, RunError> {
    Uuid::parse_str(need(v, field)?).map_err(|_| bad(field))
}

/// An id, or none when the field is empty.
pub fn id_opt(v: &Values, field: &str) -> Result<Option<Uuid>, RunError> {
    one(v, field)
        .map(|s| Uuid::parse_str(s).map_err(|_| bad(field)))
        .transpose()
}

/// An RFC 3339 time as milliseconds since the epoch.
pub fn millis_opt(v: &Values, field: &str) -> Result<Option<i64>, RunError> {
    one(v, field)
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|t| t.timestamp_millis())
                .map_err(|_| bad(field))
        })
        .transpose()
}

/// `read`, `add-members` ... as the wire right.
pub fn right(key: &str, field: &str) -> Result<Right, RunError> {
    Ok(match key {
        "read" => Right::Read,
        "add-members" => Right::AddMembers,
        "remove-members" => Right::RemoveMembers,
        "manage" => Right::Manage,
        _ => return Err(bad(field)),
    })
}

/// The rights of a list field.
pub fn rights(v: &Values, field: &str) -> Result<Vec<Right>, RunError> {
    many(v, field)
        .into_iter()
        .map(|k| right(k, field))
        .collect()
}

/// The key of a wire right.
pub fn right_key(r: Right) -> &'static str {
    match r {
        Right::Read => "read",
        Right::AddMembers => "add-members",
        Right::RemoveMembers => "remove-members",
        Right::Manage => "manage",
    }
}

/// `none`, `down`, `up`.
pub fn flow(key: &str, field: &str) -> Result<RightFlow, RunError> {
    Ok(match key {
        "none" => RightFlow::None,
        "down" => RightFlow::Down,
        "up" => RightFlow::Up,
        _ => return Err(bad(field)),
    })
}

/// The key of a wire flow.
pub fn flow_key(f: RightFlow) -> &'static str {
    match f {
        RightFlow::None => "none",
        RightFlow::Down => "down",
        RightFlow::Up => "up",
    }
}

/// `yes` / `no`.
pub fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// A `yes`/`no` choice; a missing value is `no`.
pub fn is_yes(v: &Values, field: &str) -> Result<bool, RunError> {
    match one(v, field) {
        None | Some("no") => Ok(false),
        Some("yes") => Ok(true),
        Some(_) => Err(bad(field)),
    }
}
