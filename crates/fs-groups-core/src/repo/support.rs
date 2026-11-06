//! What this provider reports it can do.
//!
//! Pattern: Null Object (report) — one place that lists every capability id of the schema; the
//! one not built says so plainly (ADR 0045).

use fs_interfaces::Capabilities;

use super::handle::GroupRepo;

/// Capability ids that `GroupRepo` provides.
const SUPPORTED: [&str; 13] = [
    "group.create",
    "group.update",
    "group.delete",
    "group.read",
    "group.nest",
    "member.add",
    "member.remove",
    "member.read",
    "invite.create",
    "invite.answer",
    "invite.read",
    "right.grant",
    "right.read",
];

impl GroupRepo {
    pub(super) fn support() -> Capabilities {
        let mut c = Capabilities::new("fs-groups-core");
        for id in SUPPORTED {
            c.supported(id);
        }
        c.unsupported("group.subscribe", "not built yet; it is possible");
        c
    }
}
