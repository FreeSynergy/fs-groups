//! What this provider reports it can do.
//!
//! Pattern: Null Object (report) — one place that lists every capability id of the schema (ADR 0045).
//! All are offered today (the `follow.*` ids are left out until a server exposes `Follows`); one that is not would be recorded here with its reason.

use fs_interfaces::Capabilities;

use super::handle::GroupRepo;

/// Capability ids that `GroupRepo` provides.
const SUPPORTED: [&str; 14] = [
    "group.create",
    "group.update",
    "group.delete",
    "group.read",
    "group.nest",
    "group.subscribe",
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
        c
    }
}
