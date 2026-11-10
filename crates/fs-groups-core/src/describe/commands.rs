//! The one table of commands of the program: every command of the core and of the contract `groups.capnp`.
//!
//! Pattern: Registry (table) — the single source. The type description (`types.rs`), the list of commands
//! the program cannot run yet ([`unsupported_commands`]) and the tests are all derived from [`COMMANDS`]; nothing
//! lists a command a second time. A command that is missing is **in** the table with `supported = false`
//! (never left out), so the frontends show it locked and it switches on when the flag turns true.
//!
//! Rules that come from the strategy (thread T, 2026-11-10): the list of every type is `find` with an empty term
//! and paging; **change** is typed field by field (missing = unchanged, empty = cleared); a command on one record
//! has an input that is a reference to its own type. There is no patch command and no list command here.
//!
//! `covers` names the methods of the contract (`Interface.method`, as in `groups.capnp`) and of the core
//! (`Type::method`) a command stands for; `change` stands for three contract methods. A method that is no
//! command at all is in [`NOT_COMMANDS`] with the reason.

use fs_types::BasicType as B;
use fs_types::CommandKind as K;

use super::arg::{Arg, FLOWS, JOIN_POLICIES, RIGHTS, YES_NO};
use super::fields::{
    DELEGATION, DELIVERY, GROUP, INVITATION, MEMBERSHIP, PARENT_REQUEST, SOURCE,
};

/// One command of one data type.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    /// The data type it belongs to.
    pub data_type: &'static str,
    /// Machine name, unique per type.
    pub name: &'static str,
    /// The kind (the frontend places and confirms by kind).
    pub kind: K,
    /// Whether this program can run it now. `false`: in the description, refused by the backend.
    pub supported: bool,
    /// The methods of the contract and the core this command stands for.
    pub covers: &'static [&'static str],
    /// The inputs.
    pub input: &'static [Arg],
    /// The results.
    pub result: &'static [Arg],
}

const fn id(of: &'static str) -> Arg {
    Arg::reference("id", of).req()
}
const fn group_req() -> Arg {
    Arg::reference("group", GROUP).req()
}
const fn group_opt() -> Arg {
    // The contract reads per group; with no group the backend reads every group.
    Arg::reference("group", GROUP)
}
const fn person_req() -> Arg {
    Arg::reference("person", "Person").req()
}
const fn text(name: &'static str) -> Arg {
    Arg::new(name, B::Text)
}
const fn cursor() -> Arg {
    text("cursor")
}
const fn next() -> Arg {
    text("next")
}

/// Every command, per data type.
#[rustfmt::skip]
pub static COMMANDS: &[Command] = &[
    // ---- Group ----
    Command { data_type: GROUP, name: "create", kind: K::Create, supported: true, covers: &["Items.create"],
        input: &[Arg::new("name", B::Text).req(), text("kind")],
        result: &[id(GROUP)] },
    // Typed change; the backend maps the fields that are present to rename, setKind and setRightFlow.
    Command { data_type: GROUP, name: "change", kind: K::Change, supported: true,
        covers: &["Items.rename", "Items.setKind", "Items.setRightFlow"],
        input: &[id(GROUP), Arg::new("name", B::Text), text("kind"), Arg::choice("rightFlow", FLOWS)],
        result: &[] },
    // The list: an empty term lists everything. The contract pages by cursor; the term is applied by the backend.
    Command { data_type: GROUP, name: "find", kind: K::Find, supported: true, covers: &["Items.list", "Items.get"],
        input: &[text("term"), cursor()],
        result: &[Arg::reference("groups", GROUP).many(), next()] },
    Command { data_type: GROUP, name: "delete", kind: K::Delete, supported: true, covers: &["Items.delete"],
        input: &[id(GROUP)], result: &[] },
    Command { data_type: GROUP, name: "parents", kind: K::Other, supported: true, covers: &["Items.parents"],
        input: &[id(GROUP)], result: &[Arg::reference("ids", GROUP).many()] },
    Command { data_type: GROUP, name: "children", kind: K::Other, supported: true, covers: &["Items.children"],
        input: &[id(GROUP)], result: &[Arg::reference("ids", GROUP).many()] },
    Command { data_type: GROUP, name: "ancestors", kind: K::Other, supported: true, covers: &["Items.ancestors"],
        input: &[id(GROUP)], result: &[Arg::reference("ids", GROUP).many()] },
    Command { data_type: GROUP, name: "descendants", kind: K::Other, supported: true, covers: &["Items.descendants"],
        input: &[id(GROUP)], result: &[Arg::reference("ids", GROUP).many()] },
    Command { data_type: GROUP, name: "removeParent", kind: K::Other, supported: true, covers: &["Items.removeParent"],
        input: &[id(GROUP), Arg::reference("parent", GROUP).req()], result: &[] },
    // Core `Mirror::sync_to` needs a directory provider; the contract has no method that starts it.
    Command { data_type: GROUP, name: "sync", kind: K::Other, supported: false, covers: &["Mirror::sync_to"],
        input: &[id(GROUP)], result: &[text("report").many()] },
    // Core `Mirror::divergences`; no contract method returns it ("not part of this contract" in groups.capnp).
    Command { data_type: GROUP, name: "divergences", kind: K::Other, supported: false, covers: &["Mirror::divergences"],
        input: &[id(GROUP)], result: &[text("divergences").many()] },
    // groups.md, question 3 (decided 2026-11-07): each group sets how one joins. Neither core nor contract has it.
    Command { data_type: GROUP, name: "setJoinPolicy", kind: K::Other, supported: false, covers: &["groups.md: join policy"],
        input: &[id(GROUP), Arg::choice("policy", JOIN_POLICIES).req()], result: &[] },
    // ---- ParentRequest ----
    Command { data_type: PARENT_REQUEST, name: "create", kind: K::Create, supported: true, covers: &["Items.requestParent"],
        input: &[Arg::reference("child", GROUP).req(), Arg::reference("parent", GROUP).req()],
        result: &[id(PARENT_REQUEST)] },
    // The contract reads per parent; with no parent the backend reads every group.
    Command { data_type: PARENT_REQUEST, name: "find", kind: K::Find, supported: true, covers: &["Items.parentRequests"],
        input: &[Arg::reference("parent", GROUP)], result: &[Arg::reference("requests", PARENT_REQUEST).many()] },
    Command { data_type: PARENT_REQUEST, name: "approve", kind: K::Other, supported: true, covers: &["Items.approveParent"],
        input: &[id(PARENT_REQUEST)], result: &[Arg::reference("group", GROUP)] },
    Command { data_type: PARENT_REQUEST, name: "decline", kind: K::Other, supported: true, covers: &["Items.declineParent"],
        input: &[id(PARENT_REQUEST)], result: &[] },
    // ---- Membership ----
    Command { data_type: MEMBERSHIP, name: "add", kind: K::Create, supported: true, covers: &["Memberships.add"],
        input: &[group_req(), Arg::new("member", B::Reference).req(), text("role")],
        result: &[id(MEMBERSHIP)] },
    Command { data_type: MEMBERSHIP, name: "remove", kind: K::Delete, supported: true, covers: &["Memberships.remove"],
        input: &[id(MEMBERSHIP)], result: &[] },
    Command { data_type: MEMBERSHIP, name: "find", kind: K::Find, supported: true, covers: &["Memberships.direct"],
        input: &[group_opt(), cursor()],
        result: &[Arg::reference("memberships", MEMBERSHIP).many(), next()] },
    // The paths are parallel to the persons: for person n, the groups from the asked group to the direct membership.
    Command { data_type: MEMBERSHIP, name: "effective", kind: K::Other, supported: true, covers: &["Memberships.effective"],
        input: &[group_req()],
        result: &[Arg::reference("persons", "Person").many(), text("paths").many()] },
    Command { data_type: MEMBERSHIP, name: "groupsOf", kind: K::Other, supported: true, covers: &["Memberships.groupsOf"],
        input: &[person_req()], result: &[Arg::reference("ids", GROUP).many()] },
    Command { data_type: MEMBERSHIP, name: "isMember", kind: K::Check, supported: true, covers: &["Memberships.isMember"],
        input: &[group_req(), person_req()], result: &[Arg::choice("member", YES_NO)] },
    // groups.md, question 3: "on request with confirmation". No core, no contract.
    Command { data_type: MEMBERSHIP, name: "requestJoin", kind: K::Other, supported: false, covers: &["groups.md: join on request"],
        input: &[group_req(), person_req()], result: &[] },
    // ---- Invitation ----
    Command { data_type: INVITATION, name: "invite", kind: K::Create, supported: true, covers: &["Invitations.invite"],
        input: &[group_req(), text("invitee").req(), text("role"), Arg::new("expires", B::DateTime)],
        result: &[id(INVITATION)] },
    Command { data_type: INVITATION, name: "accept", kind: K::Other, supported: true, covers: &["Invitations.accept"],
        input: &[id(INVITATION), person_req()], result: &[Arg::reference("membership", MEMBERSHIP)] },
    Command { data_type: INVITATION, name: "acceptProved", kind: K::Other, supported: true, covers: &["Invitations.acceptProved"],
        input: &[id(INVITATION), person_req(), Arg::new("provedAddress", B::Email).req()],
        result: &[Arg::reference("membership", MEMBERSHIP)] },
    Command { data_type: INVITATION, name: "decline", kind: K::Other, supported: true, covers: &["Invitations.decline"],
        input: &[id(INVITATION)], result: &[] },
    Command { data_type: INVITATION, name: "revoke", kind: K::Other, supported: true, covers: &["Invitations.revoke"],
        input: &[id(INVITATION)], result: &[] },
    Command { data_type: INVITATION, name: "find", kind: K::Find, supported: true, covers: &["Invitations.list"],
        input: &[group_opt()], result: &[Arg::reference("invitations", INVITATION).many()] },
    // Core `Invitations::expire_due` (a sweep, an addition to the contract); no contract method.
    Command { data_type: INVITATION, name: "expireDue", kind: K::Other, supported: false, covers: &["Invitations::expire_due"],
        input: &[], result: &[text("expired")] },
    // ---- Delegation ----
    Command { data_type: DELEGATION, name: "grant", kind: K::Create, supported: true, covers: &["Delegations.grant"],
        input: &[group_req(), Arg::new("holder", B::Reference).req(), Arg::choice("right", RIGHTS).req()],
        result: &[id(DELEGATION)] },
    Command { data_type: DELEGATION, name: "revoke", kind: K::Delete, supported: true, covers: &["Delegations.revoke"],
        input: &[id(DELEGATION)], result: &[] },
    Command { data_type: DELEGATION, name: "find", kind: K::Find, supported: true, covers: &["Delegations.list"],
        input: &[group_opt()], result: &[Arg::reference("delegations", DELEGATION).many()] },
    Command { data_type: DELEGATION, name: "mayDo", kind: K::Check, supported: true, covers: &["Delegations.mayDo"],
        input: &[group_req(), person_req(), Arg::choice("right", RIGHTS).req()],
        result: &[Arg::choice("allowed", YES_NO)] },
    // ---- Delivery (following) ----
    Command { data_type: DELIVERY, name: "deliver", kind: K::Create, supported: true, covers: &["Follows.deliver"],
        input: &[group_req(), Arg::new("recipient", B::Link).req(), Arg::choice("rights", RIGHTS).many().req(),
                 Arg::choice("passOn", YES_NO)],
        result: &[id(DELIVERY)] },
    Command { data_type: DELIVERY, name: "relay", kind: K::Other, supported: true, covers: &["Follows.passOn"],
        input: &[Arg::reference("source", SOURCE).req(), Arg::new("recipient", B::Link).req(),
                 Arg::choice("rights", RIGHTS).many().req()],
        result: &[id(DELIVERY)] },
    Command { data_type: DELIVERY, name: "revoke", kind: K::Delete, supported: true, covers: &["Follows.revoke"],
        input: &[id(DELIVERY)], result: &[] },
    Command { data_type: DELIVERY, name: "find", kind: K::Find, supported: true, covers: &["Follows.deliveries"],
        input: &[group_opt()], result: &[Arg::reference("deliveries", DELIVERY).many()] },
    // ---- Source (following) ----
    Command { data_type: SOURCE, name: "receive", kind: K::Create, supported: true, covers: &["Follows.receive"],
        input: &[Arg::new("sender", B::Link).req(), text("delivery").req(), Arg::new("source", B::Link).req(),
                 Arg::new("recipient", B::Link).req(), Arg::choice("rights", RIGHTS).many().req(),
                 Arg::choice("passOn", YES_NO), Arg::new("origin", B::Link)],
        result: &[id(SOURCE)] },
    Command { data_type: SOURCE, name: "withdraw", kind: K::Other, supported: true, covers: &["Follows.withdraw"],
        input: &[Arg::new("sender", B::Link).req(), Arg::new("source", B::Link).req(), text("delivery").req()],
        result: &[Arg::reference("id", SOURCE)] },
    Command { data_type: SOURCE, name: "remove", kind: K::Delete, supported: true, covers: &["Follows.removeSource"],
        input: &[id(SOURCE)], result: &[] },
    Command { data_type: SOURCE, name: "find", kind: K::Find, supported: true, covers: &["Follows.sources"],
        input: &[group_opt()], result: &[Arg::reference("sources", SOURCE).many()] },
];

/// Methods that are no command, and why.
pub const NOT_COMMANDS: &[(&str, &str)] = &[
    ("Groups.supportReport", "capability report, answered by the server itself"),
    ("Groups.items", "hands out a sub-capability"),
    ("Groups.memberships", "hands out a sub-capability"),
    ("Groups.invitations", "hands out a sub-capability"),
    ("Groups.delegations", "hands out a sub-capability"),
    ("Groups.follows", "hands out a sub-capability"),
    ("Groups.subscribe", "a stream of changes, not a command on a record; the frontend reads again instead"),
    ("Groups.types", "this description itself"),
    ("Items.addParent", "superseded by requestParent; every provider answers unsupported"),
    ("GroupDirectory.*", "provider interface towards a directory; fs-groups is its client"),
    ("Groups::capabilities", "core: capability report"),
];

/// The commands of one data type, in table order.
pub fn commands_of(data_type: &str) -> impl Iterator<Item = &'static Command> + '_ {
    COMMANDS.iter().filter(move |c| c.data_type == data_type)
}

/// The commands this program cannot run yet, as `Type.command`.
///
/// The wire description cannot carry the flag yet (fs-types has no `supported`), so this is how a backend
/// knows what to refuse.
pub fn unsupported_commands() -> Vec<String> {
    COMMANDS
        .iter()
        .filter(|c| !c.supported)
        .map(|c| format!("{}.{}", c.data_type, c.name))
        .collect()
}

/// The command `name` of `data_type`.
pub fn find_command(data_type: &str, name: &str) -> Option<&'static Command> {
    commands_of(data_type).find(|c| c.name == name)
}
