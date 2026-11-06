//! Ids, text and what fits into one answer.
//!
//! Pattern: Adapter — request fields into Rust, with their failures as typed problems. Limits, because
//! one message over the limit ends the whole connection (`fs_interfaces::rpc`): one text field cannot
//! grow an answer past a message, and a list without a cursor refuses rather than overflows.

use fs_interfaces::common_capnp::{cursor, uuid};
use fs_interfaces::rpc::{
    check_answer, check_intake, invalid_request, read_object_id, too_large, write_object_id,
};
use fs_store::ObjectId;

/// What one stored character can come back as: a group repeats its name and kind (envelope and AS2),
/// and a control character in the AS2 JSON takes six bytes. So `check_intake` is asked about the
/// worst case; together that is about 1 MiB per field, far above any real name or IRI. Provisional.
const WORST_CASE_FACTOR: usize = 16;
/// Longest person, invitee, role or address accepted (provisional): an IRI fits, and a list of them
/// stays readable. Longer values could fill an answer with few entries.
const MAX_IDENTIFIER_BYTES: usize = 4 * 1024;
/// Fixed space of one list entry on the wire, on top of its text.
pub(super) const ENTRY_BYTES: usize = 64;
/// Space of one id in a list.
pub(super) const ID_BYTES: usize = 16;

/// A part of a request that cannot be read (a malformed pointer) is the request's fault.
pub(crate) fn request<T>(part: capnp::Result<T>) -> capnp::Result<T> {
    part.map_err(|e| invalid_request(e.extra))
}

/// An id of a request.
pub(crate) fn read_id(id: capnp::Result<uuid::Reader<'_>>) -> capnp::Result<ObjectId> {
    read_object_id(request(id)?)
}

/// The cursor of a request; none means the start.
pub(crate) fn read_cursor(cursor: capnp::Result<cursor::Reader<'_>>) -> capnp::Result<&[u8]> {
    request(request(cursor)?.get_opaque())
}

/// A text field of a request; it is refused before anything is stored if it could not come back.
pub(crate) fn read_text(text: capnp::Result<capnp::text::Reader<'_>>) -> capnp::Result<&str> {
    let text = request(text)?
        .to_str()
        .map_err(|e| invalid_request(e.to_string()))?;
    check_intake(text.len().saturating_mul(WORST_CASE_FACTOR))?;
    Ok(text)
}

/// A person, invitee, role or address of a request.
pub(crate) fn read_identifier(text: capnp::Result<capnp::text::Reader<'_>>) -> capnp::Result<&str> {
    let text = read_text(text)?;
    if text.len() > MAX_IDENTIFIER_BYTES {
        return Err(too_large(format!(
            "{} bytes, an identifier has at most {MAX_IDENTIFIER_BYTES}",
            text.len()
        )));
    }
    Ok(text)
}

/// The length of a list in one answer, if `bytes` of it fit into one answer.
pub(super) fn list_len(len: usize, bytes: usize) -> capnp::Result<u32> {
    check_answer(bytes)?;
    u32::try_from(len).map_err(|_| too_large("list too long"))
}

/// Write a list of ids; `init` makes the list once its length is known to fit.
pub(crate) fn write_ids<'a>(
    ids: &[ObjectId],
    init: impl FnOnce(u32) -> capnp::struct_list::Builder<'a, uuid::Owned>,
) -> capnp::Result<()> {
    let mut out = init(list_len(ids.len(), ids.len().saturating_mul(ID_BYTES))?);
    for (i, id) in (0u32..).zip(ids) {
        write_object_id(out.reborrow().get(i), *id);
    }
    Ok(())
}
