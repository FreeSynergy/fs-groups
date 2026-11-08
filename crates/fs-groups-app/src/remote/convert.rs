//! Conversions from the wire: errors, ids and groups.
//!
//! Pattern: Adapter — turns Cap'n Proto readers and problems into the window's own types.

use fs_interfaces::groups_capnp::group;
use fs_interfaces::rpc::{Problem, read_envelope, read_uuid};
use uuid::Uuid;

use super::model::{ApiError, Group};

pub(super) fn problem(error: &capnp::Error) -> ApiError {
    match Problem::from_error(error) {
        Some(p) if p.is_unsupported() => ApiError::Unsupported(p.detail),
        Some(p) if p.status == 404 => ApiError::NotFound(p.detail),
        Some(p) => ApiError::Failed(p.detail),
        None => ApiError::Failed(error.to_string()),
    }
}

pub(super) fn parse_id(id: &str) -> Result<Uuid, ApiError> {
    id.parse()
        .map_err(|_| ApiError::NotFound(format!("\"{id}\" names no group.")))
}

pub(super) fn read_group(g: group::Reader<'_>) -> capnp::Result<Group> {
    let envelope = read_envelope(g.get_envelope()?)?;
    let parents = g
        .get_parents()?
        .iter()
        .filter_map(read_uuid)
        .map(|u| u.to_string())
        .collect();
    Ok(Group {
        id: envelope.id.to_string(),
        name: envelope.name,
        kind: g.get_kind()?.to_string()?,
        parents,
    })
}
