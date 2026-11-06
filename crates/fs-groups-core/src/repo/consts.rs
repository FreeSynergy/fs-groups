//! Constants of the groups objects.
//!
//! Pattern: Registry (constants) — AS2 types, context and media type used by this program in one place.

/// AS2 type of a group envelope (ADR 0020).
pub const GROUP_TYPE: &str = "Group";
/// AS2 type of a membership (`org:Membership` as a `Relationship`).
pub const MEMBERSHIP_TYPE: &str = "Relationship";
/// AS2 type of an invitation (an `Invite` activity).
pub const INVITATION_TYPE: &str = "Invite";
/// Type of a delegation in the store; AS2 has none.
pub const DELEGATION_TYPE: &str = "Delegation";
/// `context` of all objects of this program.
pub const CONTEXT: &str = "groups";
pub(super) const LEDGER_TYPE: &str = "MirrorLedger";
pub(super) const AS2_TYPE: &str = "application/activity+json";
pub(super) const ORG_MEMBERSHIP: &str = "org:Membership";
