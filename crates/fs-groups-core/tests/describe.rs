//! The type descriptions against the traits they describe (`DescribesType`, `TypeRegistry`) and against the
//! requirements: every command of the contract, of the core and of `docs/en/programs/platform/groups.md` is in
//! the table, and the description is built from the whole table.

use std::collections::BTreeSet;

use fs_groups_core::describe::{
    COMMANDS, NOT_COMMANDS, TYPES, commands_of, find_command, registry, unsupported_commands,
};
use fs_groups_core::{Delegation, Delivery, Group, Invitation, Membership, ParentRequest, Source};
use fs_types::{BasicType, CommandKind, DescribesType};

/// Every method of `groups.capnp` (fs-interfaces v0.0.8), by interface. Written from the schema; when the schema
/// grows, a new method has to be added here and to the table, which is the point of the test.
const CONTRACT: &[&str] = &[
    "Groups.supportReport",
    "Groups.items",
    "Groups.memberships",
    "Groups.invitations",
    "Groups.delegations",
    "Groups.subscribe",
    "Groups.follows",
    "Groups.types",
    "Items.create",
    "Items.rename",
    "Items.setKind",
    "Items.delete",
    "Items.get",
    "Items.list",
    "Items.addParent",
    "Items.removeParent",
    "Items.parents",
    "Items.children",
    "Items.ancestors",
    "Items.descendants",
    "Items.setRightFlow",
    "Items.requestParent",
    "Items.approveParent",
    "Items.declineParent",
    "Items.parentRequests",
    "Memberships.add",
    "Memberships.remove",
    "Memberships.direct",
    "Memberships.effective",
    "Memberships.groupsOf",
    "Memberships.isMember",
    "Invitations.invite",
    "Invitations.accept",
    "Invitations.decline",
    "Invitations.revoke",
    "Invitations.list",
    "Invitations.acceptProved",
    "Delegations.grant",
    "Delegations.revoke",
    "Delegations.list",
    "Delegations.mayDo",
    "Follows.deliver",
    "Follows.passOn",
    "Follows.revoke",
    "Follows.deliveries",
    "Follows.receive",
    "Follows.withdraw",
    "Follows.sources",
    "Follows.removeSource",
];

/// Every method of the core traits that is no read of the same thing as a contract method, as the table names it.
/// (The traits `Groups`, `Memberships`, `Invitations`, `Delegations`, `Follows` of `fs-groups-core`; the ones that
/// mirror a contract method are listed with it in `covers`.)
const CORE_ONLY: &[&str] = &[
    "Invitations::expire_due",
    "Mirror::sync_to",
    "Mirror::divergences",
];

/// What `groups.md` promises a person can do ("You can create, nest and delete groups, add members with a role, send
/// invitations that expire, and delegate and revoke rights", mirroring, following, joining by the group's rule), as
/// `Type.command`.
const REQUIRED: &[&str] = &[
    "Group.create",
    "Group.delete",
    "Group.change",
    "ParentRequest.create",
    "ParentRequest.approve",
    "Membership.add",
    "Membership.remove",
    "Membership.effective",
    "Invitation.invite",
    "Invitation.accept",
    "Invitation.revoke",
    "Delegation.grant",
    "Delegation.revoke",
    "Delegation.mayDo",
    "Group.sync",
    "Delivery.deliver",
    "Delivery.revoke",
    "Source.receive",
    "Source.remove",
    "Group.setJoinPolicy",
    "Membership.requestJoin",
];

fn covered() -> BTreeSet<&'static str> {
    COMMANDS
        .iter()
        .flat_map(|c| c.covers.iter().copied())
        .collect()
}

#[test]
fn registry_holds_every_type_and_validates() {
    let reg = registry().unwrap();
    assert_eq!(reg.len(), TYPES.len());
    for name in TYPES {
        let d = reg.get(name).unwrap();
        assert_eq!(d.program, "fs-groups");
        d.validate().unwrap();
    }
    // Every reference points to a type of this program or to People.
    for (_, _, target) in reg.unresolved_targets() {
        assert_eq!(target, "Person");
    }
}

#[test]
fn describes_type_by_trait_matches_the_tables() {
    assert_eq!(Group::describe().name, "Group");
    assert_eq!(Group::describe().standard, "AS2 Group");
    assert_eq!(Membership::describe().name, "Membership");
    assert_eq!(Invitation::describe().name, "Invitation");
    assert_eq!(Delegation::describe().name, "Delegation");
    assert_eq!(ParentRequest::describe().name, "ParentRequest");
    assert_eq!(Delivery::describe().name, "Delivery");
    assert_eq!(Source::describe().name, "Source");
}

#[test]
fn every_contract_method_is_a_command_or_named_as_none() {
    let covered = covered();
    for m in CONTRACT {
        let none = NOT_COMMANDS.iter().any(|(n, _)| n == m);
        assert!(
            covered.contains(m) || none,
            "contract method {m} is not in the table"
        );
    }
    for (n, why) in NOT_COMMANDS {
        assert!(!why.is_empty(), "{n} needs a reason");
    }
}

#[test]
fn every_core_only_method_is_in_the_table_unsupported() {
    for m in CORE_ONLY {
        let c = COMMANDS
            .iter()
            .find(|c| c.covers.contains(m))
            .unwrap_or_else(|| panic!("{m} missing"));
        assert!(
            !c.supported,
            "{m} has no contract method, so this backend cannot run it"
        );
    }
}

#[test]
fn every_requirement_is_in_the_table_and_the_description() {
    let reg = registry().unwrap();
    for r in REQUIRED {
        let (t, c) = r.split_once('.').unwrap();
        assert!(find_command(t, c).is_some(), "{r} is not in the table");
        assert!(
            reg.get(t).unwrap().find_command(c).is_some(),
            "{r} is not in the description"
        );
    }
}

#[test]
fn the_description_holds_all_commands_also_the_unsupported() {
    let reg = registry().unwrap();
    let described: usize = reg.iter().map(|t| t.commands.len()).sum();
    assert_eq!(described, COMMANDS.len());
    let unsupported = unsupported_commands();
    assert!(!unsupported.is_empty());
    for u in &unsupported {
        let (t, c) = u.split_once('.').unwrap();
        let cmd = reg
            .get(t)
            .unwrap()
            .find_command(c)
            .expect("in the description");
        assert!(!cmd.supported, "{u} is locked on the wire");
    }
    // ... and every other command is free.
    let locked: usize = reg
        .iter()
        .flat_map(|t| &t.commands)
        .filter(|c| !c.supported)
        .count();
    assert_eq!(locked, unsupported.len());
    assert_eq!(
        unsupported,
        [
            "Group.sync",
            "Group.divergences",
            "Group.setJoinPolicy",
            "Membership.requestJoin",
            "Invitation.expireDue",
        ]
    );
}

#[test]
fn every_type_has_a_find_and_the_record_rules_hold() {
    for t in TYPES {
        assert!(
            find_command(t, "find").is_some_and(|c| c.kind == CommandKind::Find),
            "{t} has no find"
        );
        // No list command and no patch command (strategy decision 2026-11-10).
        assert!(commands_of(t).all(|c| c.name != "list" && c.name != "patch"));
        for c in commands_of(t) {
            let has_own_ref = c
                .input
                .iter()
                .any(|a| a.basic == BasicType::Reference && a.target == *t);
            match c.kind {
                // Change and delete work on one record: the input is a reference to the own type.
                CommandKind::Delete => assert!(has_own_ref, "{t}.{} needs its record", c.name),
                // Change is an Edit: no input list (rule of types.capnp, checked by validate).
                CommandKind::Change => assert!(c.input.is_empty()),
                CommandKind::Create | CommandKind::Find => {}
                _ => {}
            }
        }
    }
}

#[test]
fn record_commands_are_the_ones_with_the_record_as_input() {
    let g = Group::describe();
    let names: Vec<&str> = g
        .record_commands()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    for n in [
        "change",
        "delete",
        "parents",
        "children",
        "removeParent",
        "sync",
    ] {
        assert!(names.contains(&n), "{n}");
    }
    assert!(!names.contains(&"create") && !names.contains(&"find"));
}

#[test]
fn find_is_the_standard_find() {
    for t in TYPES {
        let d = registry().unwrap();
        let f = d.get(t).unwrap().find_command("find").unwrap();
        assert_eq!(f.input[0].name, "term");
        assert_eq!(f.result[0].name, "items");
        assert_eq!(f.result[0].target, *t);
    }
}

#[test]
fn choices_are_what_the_contract_has() {
    let g = Group::describe();
    let flow = g.find_field("rightFlow").unwrap();
    assert_eq!(flow.choices, ["none", "down", "up"]);
    let d = Delegation::describe();
    assert_eq!(
        d.find_field("right").unwrap().choices,
        ["read", "add-members", "remove-members", "manage"]
    );
}
