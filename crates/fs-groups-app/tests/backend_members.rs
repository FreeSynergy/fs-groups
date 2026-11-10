//! The Admin backend over a real socket: memberships, invitations, delegations.

mod common;

use common::{ALICE, World, id_of, values};
use fs_admin_frontend::{Backend, BackendError, Values};
use fs_groups_core::describe::{DELEGATION, GROUP, INVITATION, MEMBERSHIP};

#[test]
fn members_invitations_and_rights() {
    let w = World::new();
    let g = id_of(
        w.of(GROUP)
            .run("create", None, &values(&[("name", &["Club"])]))
            .unwrap(),
    );

    let members = w.of(MEMBERSHIP);
    let m = id_of(
        members
            .run(
                "add",
                None,
                &values(&[("group", &[&g]), ("member", &[ALICE]), ("role", &["chair"])]),
            )
            .unwrap(),
    );
    let rows = members.list(&values(&[("group", &[&g])])).unwrap();
    assert_eq!((rows.len(), rows[0].id.as_str()), (1, m.as_str()));
    assert_eq!(rows[0].values["member"], [ALICE]);
    assert_eq!(rows[0].values["role"], ["chair"]);
    assert_eq!(
        members.list(&Values::new()).unwrap().len(),
        1,
        "no group named: every group is asked"
    );
    let is = members
        .run(
            "isMember",
            None,
            &values(&[("group", &[&g]), ("person", &[ALICE])]),
        )
        .unwrap();
    assert_eq!(is.values["member"], ["yes"]);
    let eff = members
        .run("effective", None, &values(&[("group", &[&g])]))
        .unwrap();
    assert_eq!(eff.values["persons"], [ALICE]);
    let of = members
        .run("groupsOf", None, &values(&[("person", &[ALICE])]))
        .unwrap();
    assert_eq!(of.values["ids"], std::slice::from_ref(&g));
    members.run("remove", Some(&m), &Values::new()).unwrap();
    assert!(members.list(&Values::new()).unwrap().is_empty());

    // an invitation that expires, accepted only by the invitee
    let invitations = w.of(INVITATION);
    let inv = id_of(
        invitations
            .run(
                "invite",
                None,
                &values(&[
                    ("group", &[&g]),
                    ("invitee", &[ALICE]),
                    ("role", &["member"]),
                    ("expires", &["2099-01-01T00:00:00Z"]),
                ]),
            )
            .unwrap(),
    );
    let listed = invitations.list(&values(&[("group", &[&g])])).unwrap();
    assert_eq!(listed[0].values["state"], ["pending"]);
    assert!(listed[0].values["expires"][0].starts_with("2099-01-01T00:00:00"));
    let stranger = invitations.run(
        "accept",
        Some(&inv),
        &values(&[("person", &["https://local.invalid/users/eve"])]),
    );
    assert!(
        matches!(stranger, Err(BackendError::Rejected(_))),
        "only the invitee accepts: {stranger:?}"
    );
    let ok = invitations
        .run("accept", Some(&inv), &values(&[("person", &[ALICE])]))
        .unwrap();
    assert!(ok.values.contains_key("membership"));
    assert_eq!(
        invitations.list(&Values::new()).unwrap()[0].values["state"],
        ["accepted"]
    );
    let second = id_of(
        invitations
            .run(
                "invite",
                None,
                &values(&[("group", &[&g]), ("invitee", &["bob@example.org"])]),
            )
            .unwrap(),
    );
    invitations
        .run("revoke", Some(&second), &Values::new())
        .unwrap();
    let third = id_of(
        invitations
            .run(
                "invite",
                None,
                &values(&[("group", &[&g]), ("invitee", &["carol@example.org"])]),
            )
            .unwrap(),
    );
    invitations
        .run("decline", Some(&third), &Values::new())
        .unwrap();
    let state: Vec<_> = invitations
        .list(&Values::new())
        .unwrap()
        .into_iter()
        .map(|r| r.values["state"][0].clone())
        .collect();
    assert_eq!(state, ["accepted", "revoked", "declined"]);

    // delegation: grant, ask, revoke
    let rights = w.of(DELEGATION);
    let d = id_of(
        rights
            .run(
                "grant",
                None,
                &values(&[
                    ("group", &[&g]),
                    ("holder", &[ALICE]),
                    ("right", &["add-members"]),
                ]),
            )
            .unwrap(),
    );
    assert_eq!(
        rights.list(&values(&[("group", &[&g])])).unwrap()[0].values["right"],
        ["add-members"]
    );
    let may = rights
        .run(
            "mayDo",
            None,
            &values(&[
                ("group", &[&g]),
                ("person", &[ALICE]),
                ("right", &["add-members"]),
            ]),
        )
        .unwrap();
    assert_eq!(may.values["allowed"], ["yes"]);
    let may_not = rights
        .run(
            "mayDo",
            None,
            &values(&[
                ("group", &[&g]),
                ("person", &[ALICE]),
                ("right", &["manage"]),
            ]),
        )
        .unwrap();
    assert_eq!(may_not.values["allowed"], ["no"]);
    rights.run("revoke", Some(&d), &Values::new()).unwrap();
    assert!(rights.list(&Values::new()).unwrap().is_empty());
}
