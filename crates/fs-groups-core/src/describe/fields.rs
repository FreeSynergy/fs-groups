//! The fields of every data type of the program, as one table.
//!
//! Pattern: Registry (table) — the shape of each stored record: what the Admin frontend lists and edits. A
//! reference to a group is the group's id as text. The standard of each type is named in `types.rs`.
//! Not fields (they live elsewhere): the record's own id (the frontend gets it as `Record::id`).

use fs_types::BasicType as B;

use super::arg::{
    Arg, DELIVERY_STATES, FLOWS, INVITATION_STATES, REQUEST_STATES, RIGHTS, SOURCE_STATES, YES_NO,
};

/// The data types, by name (ADR 0050: all belong to `fs-groups`).
pub const GROUP: &str = "Group";
/// `org:Membership`.
pub const MEMBERSHIP: &str = "Membership";
/// An invitation.
pub const INVITATION: &str = "Invitation";
/// A delegated right.
pub const DELEGATION: &str = "Delegation";
/// A request to become a child of a group.
pub const PARENT_REQUEST: &str = "ParentRequest";
/// A delivery of rights to another group (following).
pub const DELIVERY: &str = "Delivery";
/// A source a group entered (following).
pub const SOURCE: &str = "Source";
/// The program that owns them.
pub const PROGRAM: &str = "fs-groups";

/// All type names, in the order the catalog lists them.
pub const TYPES: &[&str] = &[
    GROUP,
    MEMBERSHIP,
    INVITATION,
    DELEGATION,
    PARENT_REQUEST,
    DELIVERY,
    SOURCE,
];

const GROUP_FIELDS: &[Arg] = &[
    Arg::new("name", B::Text).req(),
    // Free text: team, project, department ... not a fixed list.
    Arg::new("kind", B::Text),
    Arg::reference("parents", GROUP).many(),
    Arg::choice("rightFlow", FLOWS),
];

const MEMBERSHIP_FIELDS: &[Arg] = &[
    Arg::reference("group", GROUP).req(),
    // A person (IRI) or, for a holder of rights, a group id: `any` target.
    Arg::new("member", B::Reference).req(),
    // Empty = plain member; otherwise a role id of fs-people (the definition stays there).
    Arg::new("role", B::Text),
    // False when an admin role is held by an agent or someone not known to be human (ADR 0056).
    Arg::choice("inForce", YES_NO),
];

const INVITATION_FIELDS: &[Arg] = &[
    Arg::reference("group", GROUP).req(),
    // An IRI or an e-mail address.
    Arg::new("invitee", B::Text).req(),
    Arg::new("role", B::Text),
    Arg::choice("state", INVITATION_STATES),
    Arg::new("expires", B::DateTime),
];

const DELEGATION_FIELDS: &[Arg] = &[
    Arg::reference("group", GROUP).req(),
    Arg::new("holder", B::Reference).req(),
    Arg::choice("right", RIGHTS).req(),
    Arg::choice("inForce", YES_NO),
];

const PARENT_REQUEST_FIELDS: &[Arg] = &[
    Arg::reference("child", GROUP).req(),
    Arg::reference("parent", GROUP).req(),
    Arg::reference("requester", "Person"),
    Arg::choice("state", REQUEST_STATES),
];

const DELIVERY_FIELDS: &[Arg] = &[
    Arg::reference("group", GROUP).req(),
    Arg::new("recipient", B::Link).req(),
    Arg::choice("rights", RIGHTS).many(),
    Arg::choice("passOn", YES_NO),
    Arg::reference("via", SOURCE).many(),
    Arg::choice("state", DELIVERY_STATES),
    Arg::new("origin", B::Link),
];

const SOURCE_FIELDS: &[Arg] = &[
    Arg::reference("group", GROUP).req(),
    Arg::new("source", B::Link).req(),
    Arg::new("recipient", B::Link),
    Arg::choice("rights", RIGHTS).many(),
    Arg::choice("passOn", YES_NO),
    Arg::new("origin", B::Link),
    Arg::new("delivery", B::Text),
    Arg::new("sender", B::Text),
    Arg::choice("state", SOURCE_STATES),
];

/// The fields of the data type `data_type`; empty for an unknown name.
pub fn fields_of(data_type: &str) -> &'static [Arg] {
    match data_type {
        GROUP => GROUP_FIELDS,
        MEMBERSHIP => MEMBERSHIP_FIELDS,
        INVITATION => INVITATION_FIELDS,
        DELEGATION => DELEGATION_FIELDS,
        PARENT_REQUEST => PARENT_REQUEST_FIELDS,
        DELIVERY => DELIVERY_FIELDS,
        SOURCE => SOURCE_FIELDS,
        _ => &[],
    }
}
