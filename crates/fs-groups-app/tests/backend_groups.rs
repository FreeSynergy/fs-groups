//! The Admin backend over a real socket: groups, change as an Edit, nesting with consent.

mod common;

use common::{World, id_of, values};
use fs_admin_frontend::{Backend, BackendError, Values};
use fs_groups_core::describe::{GROUP, PARENT_REQUEST};
use fs_types::{BasicType, Edit};

#[test]
fn groups_create_find_change_delete_and_nesting() {
    let w = World::new();
    let groups = w.of(GROUP);
    let board = id_of(
        groups
            .run(
                "create",
                None,
                &values(&[("name", &["Board"]), ("kind", &["team"])]),
            )
            .unwrap(),
    );
    let staff = id_of(
        groups
            .run(
                "create",
                None,
                &values(&[("name", &["Staff"]), ("kind", &["department"])]),
            )
            .unwrap(),
    );

    assert_eq!(groups.list(&Values::new()).unwrap().len(), 2);
    let hit = groups.list(&values(&[("term", &["depart"])])).unwrap();
    assert_eq!(hit.len(), 1, "the kind is searched too");
    assert_eq!(hit[0].id, staff);

    // change is an Edit: name cleared is refused, kind set, flow set; fields not in the edit stay
    let edit = Edit::new()
        .set(&["name"], BasicType::Text, &["Council"])
        .set(&["rightFlow"], BasicType::Choice, &["down"]);
    groups.change(&board, &edit).unwrap();
    let after = groups.list(&values(&[("term", &["council"])])).unwrap();
    assert_eq!(after[0].values["name"], ["Council"]);
    assert_eq!(
        after[0].values["kind"],
        ["team"],
        "kind was not in the edit"
    );
    assert_eq!(after[0].values["rightFlow"], ["down"]);
    let clear_kind = Edit::new().set(&["kind"], BasicType::Text, &[]);
    groups.change(&board, &clear_kind).unwrap();
    let after = groups.list(&values(&[("term", &["council"])])).unwrap();
    assert!(
        !after[0].values.contains_key("kind"),
        "an empty kind is cleared"
    );
    let no_name = Edit::new().set(&["name"], BasicType::Text, &[]);
    assert!(matches!(
        groups.change(&board, &no_name),
        Err(BackendError::Rejected(_))
    ));

    // nesting needs the parent's consent: request (ParentRequest.create), list, approve
    let requests = w.of(PARENT_REQUEST);
    let request = id_of(
        requests
            .run(
                "create",
                None,
                &values(&[("child", &[&board]), ("parent", &[&staff])]),
            )
            .unwrap(),
    );
    let pending = requests.list(&values(&[("parent", &[&staff])])).unwrap();
    assert_eq!(
        (pending.len(), pending[0].id.as_str()),
        (1, request.as_str())
    );
    assert_eq!(pending[0].values["state"], ["pending"]);
    assert_eq!(
        requests.list(&Values::new()).unwrap().len(),
        1,
        "no parent named: every group is asked"
    );
    requests
        .run("approve", Some(&request), &Values::new())
        .unwrap();
    let parents = groups.run("parents", Some(&board), &Values::new()).unwrap();
    assert_eq!(parents.values["ids"], std::slice::from_ref(&staff));
    let children = groups
        .run("children", Some(&staff), &Values::new())
        .unwrap();
    assert_eq!(children.values["ids"], std::slice::from_ref(&board));
    let up = groups
        .run("ancestors", Some(&board), &Values::new())
        .unwrap();
    assert_eq!(up.values["ids"], std::slice::from_ref(&staff));
    let down = groups
        .run("descendants", Some(&staff), &Values::new())
        .unwrap();
    assert_eq!(down.values["ids"], std::slice::from_ref(&board));

    // a cycle is refused by the core and shows as a refusal, not as an unreachable program
    let loop_back = requests.run(
        "create",
        None,
        &values(&[("child", &[&staff]), ("parent", &[&board])]),
    );
    assert!(
        matches!(loop_back, Err(BackendError::Rejected(_))),
        "{loop_back:?}"
    );

    groups
        .run(
            "removeParent",
            Some(&board),
            &values(&[("parent", &[&staff])]),
        )
        .unwrap();
    assert!(
        groups
            .run("parents", Some(&board), &Values::new())
            .unwrap()
            .values["ids"]
            .is_empty()
    );
    groups.run("delete", Some(&board), &Values::new()).unwrap();
    assert_eq!(groups.list(&Values::new()).unwrap().len(), 1);
    assert!(matches!(
        groups.change(&board, &clear_kind),
        Err(BackendError::NotFound)
    ));
}
