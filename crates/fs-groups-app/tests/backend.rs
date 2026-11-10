//! The `Backend` of the Admin frontend over a real socket, against the store-backed server: every kind of command
//! of the table that can run, and the refusal of the ones that cannot.
//!
//! Pattern: Integration test — the server runs on its own thread and socket; the backend is the one the window uses.

use std::path::Path;
use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_groups_app::{GroupsBackend, GroupsRemote, RunError, plan_change};
use fs_groups_core::GroupRepo;
use fs_groups_core::describe::{
    COMMANDS, DELEGATION, DELIVERY, GROUP, INVITATION, MEMBERSHIP, PARENT_REQUEST, SOURCE,
    unsupported_commands,
};
use fs_groups_server::{GroupsServer, OperatorIsHuman};
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};
use fs_types::{BasicType, Edit};

const ME: &str = "https://local.invalid/users/owner";
const ALICE: &str = "https://local.invalid/users/alice";

fn start_server(socket: &Path) {
    let socket = socket.to_path_buf();
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo =
                GroupRepo::new(store, ME).with_ai_marking(Arc::new(OperatorIsHuman::new(ME)));
            let root = GroupsServer::new(Arc::new(repo)).into_client();
            let bound = bind_socket(&socket).await.unwrap();
            up.send(()).unwrap();
            serve(bound, root.client, std::future::pending()).await.ok();
        });
    });
    started.recv().unwrap();
}

fn values(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect()))
        .collect()
}

struct World {
    _dir: tempfile::TempDir,
    remote: Arc<GroupsRemote>,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("groups.sock");
        start_server(&socket);
        let remote = Arc::new(GroupsRemote::connect(&socket).unwrap());
        Self { _dir: dir, remote }
    }

    fn of(&self, data_type: &'static str) -> GroupsBackend {
        GroupsBackend::new(Arc::clone(&self.remote), data_type)
    }
}

fn id_of(o: fs_admin_frontend::Outcome) -> String {
    o.id.expect("the command names the record it made")
}

#[test]
fn the_catalog_holds_every_type_with_every_command() {
    let w = World::new();
    let catalog = w.remote.catalog().unwrap();
    assert_eq!(catalog.len(), fs_groups_core::describe::TYPES.len());
    let commands: usize = catalog.iter().map(|t| t.commands.len()).sum();
    assert_eq!(commands, COMMANDS.len());
    let locked: Vec<String> = catalog
        .iter()
        .flat_map(|t| {
            t.commands
                .iter()
                .filter(|c| !c.supported)
                .map(move |c| format!("{}.{}", t.name, c.name))
        })
        .collect();
    let mut expected = unsupported_commands();
    expected.sort();
    let mut locked = locked;
    locked.sort();
    assert_eq!(locked, expected);
}

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

#[test]
fn following_deliver_receive_withdraw() {
    let w = World::new();
    let g = id_of(
        w.of(GROUP)
            .run("create", None, &values(&[("name", &["Giver"])]))
            .unwrap(),
    );
    // a group gives only rights it holds: the acting person holds manage here
    w.of(DELEGATION)
        .run(
            "grant",
            None,
            &values(&[("group", &[&g]), ("holder", &[ME]), ("right", &["manage"])]),
        )
        .unwrap();
    let deliveries = w.of(DELIVERY);
    let d = id_of(
        deliveries
            .run(
                "deliver",
                None,
                &values(&[
                    ("group", &[&g]),
                    ("recipient", &["https://other.invalid/groups/abc"]),
                    ("rights", &["read"]),
                    ("passOn", &["no"]),
                ]),
            )
            .unwrap(),
    );
    let rows = deliveries.list(&values(&[("group", &[&g])])).unwrap();
    assert_eq!(
        (rows[0].id.as_str(), rows[0].values["state"][0].as_str()),
        (d.as_str(), "active")
    );
    deliveries.run("revoke", Some(&d), &Values::new()).unwrap();
    assert_eq!(
        deliveries.list(&Values::new()).unwrap()[0].values["state"],
        ["revoked"]
    );

    // the receiving side, the offer comes from another origin
    let receiver = id_of(
        w.of(GROUP)
            .run("create", None, &values(&[("name", &["Receiver"])]))
            .unwrap(),
    );
    let sources = w.of(SOURCE);
    let recipient = format!("https://local.invalid/groups/{receiver}");
    let s = id_of(
        sources
            .run(
                "receive",
                None,
                &values(&[
                    ("sender", &["other.invalid"]),
                    ("delivery", &[&d]),
                    ("source", &["https://other.invalid/groups/abc"]),
                    ("recipient", &[&recipient]),
                    ("rights", &["read"]),
                    ("passOn", &["no"]),
                ]),
            )
            .unwrap(),
    );
    let found = sources.list(&Values::new()).unwrap();
    assert!(found.iter().any(|r| r.id == s));
    // the giver revoked: the receiver's entry is withdrawn
    let w2 = sources
        .run(
            "withdraw",
            None,
            &values(&[
                ("sender", &["other.invalid"]),
                ("source", &["https://other.invalid/groups/abc"]),
                ("delivery", &[&d]),
            ]),
        )
        .unwrap();
    assert_eq!(w2.id.as_deref(), Some(s.as_str()));
    let state = sources
        .list(&Values::new())
        .unwrap()
        .into_iter()
        .find(|r| r.id == s)
        .unwrap();
    assert_eq!(state.values["state"], ["withdrawn"]);
    sources.run("remove", Some(&s), &Values::new()).unwrap();
}

#[test]
fn unsupported_commands_are_refused_before_any_call() {
    let w = World::new();
    for name in unsupported_commands() {
        let (t, c) = name.split_once('.').unwrap();
        let t = fs_groups_core::describe::TYPES
            .iter()
            .find(|x| **x == t)
            .unwrap();
        let backend = w.of(t);
        assert!(!backend.supported(c), "{name}");
        assert_eq!(
            backend.run_typed(c, Some("x"), &Values::new()),
            Err(RunError::Unsupported(name.clone())),
            "typed refusal"
        );
        assert_eq!(
            backend.run(c, Some("x"), &Values::new()),
            Err(BackendError::NotFound)
        );
    }
    // and the ones the table lets through are announced as supported
    assert!(w.of(GROUP).supported("create") && w.of(GROUP).supported("change"));
    assert!(!w.of(GROUP).supported("nonsense"));
    assert_eq!(
        w.of(GROUP).run_typed("nonsense", None, &Values::new()),
        Err(RunError::NotFound)
    );
}

#[test]
fn every_supported_command_of_the_table_is_wired() {
    let w = World::new();
    for c in COMMANDS
        .iter()
        .filter(|c| c.supported && c.name != "change")
    {
        let t = fs_groups_core::describe::TYPES
            .iter()
            .find(|t| **t == c.data_type)
            .unwrap();
        // An empty input is refused as a bad value (or listed), never as "in the table but not wired".
        let answer = w.of(t).run_typed(c.name, None, &Values::new());
        if let Err(RunError::Unavailable(text)) = &answer {
            assert!(
                !text.contains("not wired"),
                "{}.{}: {text}",
                c.data_type,
                c.name
            );
        }
    }
}

#[test]
fn a_change_is_mapped_field_by_field() {
    let plan = plan_change(&Edit::new().set(&["name"], BasicType::Text, &["A"]).set(
        &["kind"],
        BasicType::Text,
        &[],
    ))
    .unwrap();
    assert_eq!(plan.name.as_deref(), Some("A"));
    assert_eq!(plan.kind.as_deref(), Some(""), "cleared");
    assert!(plan.flow.is_none(), "not in the edit: unchanged");
    assert!(matches!(
        plan_change(&Edit::new().set(&["parents"], BasicType::Reference, &["x"])),
        Err(RunError::Bad { .. })
    ));
    assert!(matches!(
        plan_change(&Edit::new().set(&["rightFlow"], BasicType::Choice, &["sideways"])),
        Err(RunError::Bad { .. })
    ));
}
