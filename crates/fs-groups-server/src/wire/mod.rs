//! Groups, memberships, invitations and delegations on the wire.
//!
//! Pattern: Facade (declarations) — one file per concept of `groups.capnp`.

mod basic;
mod delegation;
mod follow;
mod group;
mod invitation;
mod member;
mod parent_request;

pub(crate) use basic::{read_cursor, read_id, read_identifier, read_text, request, write_ids};
pub(crate) use delegation::{read_right, write_delegation, write_delegations};
pub(crate) use follow::{
    read_offer, read_rights, write_deliveries, write_delivery, write_offer, write_source,
    write_sources,
};
pub(crate) use group::{read_flow, write_group, write_page};
pub(crate) use invitation::{read_expires, write_invitation, write_invitations};
pub(crate) use member::{
    effective_weight, membership_weight, read_member, write_effective, write_membership,
    write_memberships,
};
pub(crate) use parent_request::{write_parent_request, write_parent_requests};
