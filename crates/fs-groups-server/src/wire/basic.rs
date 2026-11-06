//! Ids, text and what fits into one answer.
//!
//! Pattern: Adapter — request fields into Rust, with their failures as typed problems. Limits, because
//! one message over the limit ends the whole connection (`fs_interfaces::rpc`): one text field cannot
//! grow an answer past a message, and a list without a cursor refuses rather than overflows.

use fs_interfaces::common_capnp::uuid;
use fs_interfaces::rpc::{MAX_PAGE_BYTES, read_object_id, write_object_id};
use fs_store::ObjectId;

use crate::errors::{invalid, too_large};

/// Longest text accepted in one field (provisional). A group repeats its name and kind in the AS2 text,
/// so this keeps a single group well inside a message. No legitimate name or IRI comes close.
pub(crate) const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Fixed space of one list entry on the wire, on top of its text.
pub(super) const ENTRY_BYTES: usize = 64;
const ID_BYTES: usize = 16;

/// An id of a request; none or not a valid object id is the request's fault.
pub(crate) fn read_id(id: capnp::Result<uuid::Reader<'_>>) -> capnp::Result<ObjectId> {
    read_object_id(id?).map_err(|e| invalid(e.extra))
}

/// A text field of a request.
pub(crate) fn read_text<'a>(text: capnp::Result<capnp::text::Reader<'a>>) -> capnp::Result<&'a str> {
    let text = text?.to_str().map_err(|e| invalid(e.to_string()))?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(invalid(format!(
            "a text field holds at most {MAX_TEXT_BYTES} bytes, got {}",
            text.len()
        )));
    }
    Ok(text)
}

/// The length of a list without a cursor, if `bytes` of it fit into one answer.
pub(super) fn list_len(len: usize, bytes: usize) -> capnp::Result<u32> {
    if bytes > MAX_PAGE_BYTES {
        return Err(too_large(format!(
            "{len} entries of {bytes} bytes do not fit one answer, and this list has no cursor"
        )));
    }
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
