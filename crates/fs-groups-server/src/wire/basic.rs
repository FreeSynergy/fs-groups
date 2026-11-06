//! Ids, text and what fits into one answer.
//!
//! Pattern: Adapter — request fields into Rust, with their failures as typed problems. Limits, because
//! one message over the limit ends the whole connection (`fs_interfaces::rpc`): one text field cannot
//! grow an answer past a message, and a list without a cursor refuses rather than overflows.

use fs_interfaces::common_capnp::uuid;
use fs_interfaces::rpc::{
    check_answer, check_intake, invalid_request, read_object_id, too_large, write_object_id,
};
use fs_store::ObjectId;

/// What one stored character can come back as: a group repeats its name and kind (envelope and AS2),
/// and a control character in the AS2 JSON takes six bytes. So `check_intake` is asked about the
/// worst case; together that is about 1 MiB per field, far above any real name or IRI. Provisional.
const WORST_CASE_FACTOR: usize = 16;
/// Fixed space of one list entry on the wire, on top of its text.
pub(super) const ENTRY_BYTES: usize = 64;
/// Space of one id in a list.
pub(super) const ID_BYTES: usize = 16;

/// An id of a request.
pub(crate) fn read_id(id: capnp::Result<uuid::Reader<'_>>) -> capnp::Result<ObjectId> {
    read_object_id(id?)
}

/// A text field of a request; it is refused before anything is stored if it could not come back.
pub(crate) fn read_text(text: capnp::Result<capnp::text::Reader<'_>>) -> capnp::Result<&str> {
    let text = text?.to_str().map_err(|e| invalid_request(e.to_string()))?;
    check_intake(text.len().saturating_mul(WORST_CASE_FACTOR))?;
    Ok(text)
}

/// The length of a list without a cursor, if `bytes` of it fit into one answer.
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
