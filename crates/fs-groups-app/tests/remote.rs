//! The window's adapter over the socket: `GroupsRemote` against a real server and a real store.
//!
//! Pattern: Integration test — the server runs on its own thread and socket, the adapter is the one
//! the window uses.

use std::path::Path;
use std::sync::Arc;

use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::people::PeopleSource;
use fs_groups_app::{GroupsAdapter, GroupsApi, GroupsRemote};
use fs_groups_core::GroupRepo;
use fs_groups_server::{GroupsServer, OperatorIsHuman};
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};

const ME: &str = "https://local.invalid/users/owner";

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

fn text(sheet: &fs_components_workbenches::people::PersonSheet) -> String {
    sheet
        .sections
        .iter()
        .flat_map(|s| s.fields.iter().map(|f| format!("{}={}", f.label, f.value)))
        .collect::<Vec<_>>()
        .join(";")
}

#[test]
fn create_list_read_rename_set_kind_delete_over_the_socket() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
    start_server(&socket);
    let remote = Arc::new(GroupsRemote::connect(&socket).unwrap());
    let adapter = GroupsAdapter::new(remote.clone());

    let mut draft = adapter.blank().unwrap();
    for f in draft.groups.iter_mut().flat_map(|g| g.fields.iter_mut()) {
        f.value = if f.key == "name" { "Board" } else { "team" }.into();
    }
    let key = adapter.create(&draft).unwrap();
    remote.create("Staff", "department").unwrap();

    let all = adapter.rows("", 0, 100).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(adapter.rows("board", 0, 100).unwrap().len(), 1);
    assert_eq!(
        adapter.rows("depart", 0, 100).unwrap().len(),
        1,
        "the kind is searched too"
    );
    assert_eq!(adapter.rows("", 1, 100).unwrap().len(), 1, "offset");

    let sheet = adapter.read(&key).unwrap();
    assert_eq!(sheet.title, "Board");
    assert!(text(&sheet).contains("Kind=team"), "{}", text(&sheet));

    let mut edit = adapter.draft(&key).unwrap();
    for f in edit.groups.iter_mut().flat_map(|g| g.fields.iter_mut()) {
        f.value = if f.key == "name" {
            "Council"
        } else {
            "committee"
        }
        .into();
    }
    adapter.save(&key, &edit).unwrap();
    let sheet = adapter.read(&key).unwrap();
    assert_eq!(sheet.title, "Council");
    assert!(text(&sheet).contains("Kind=committee"));

    adapter.delete(&key).unwrap();
    assert_eq!(adapter.rows("", 0, 100).unwrap().len(), 1);
    assert_eq!(adapter.read(&key).unwrap_err().kind, ErrorKind::NotFound);
}

#[test]
fn a_group_without_a_name_is_refused_before_it_reaches_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
    start_server(&socket);
    let adapter = GroupsAdapter::new(Arc::new(GroupsRemote::connect(&socket).unwrap()));
    let blank = adapter.blank().unwrap();
    assert!(adapter.create(&blank).is_err());
    assert_eq!(
        adapter.rows("", 0, 10).unwrap(),
        [] as [fs_components_workbenches::people::PersonRow; 0]
    );
}

#[test]
fn nothing_on_the_socket_is_said_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let message = GroupsRemote::connect(&dir.path().join("none.sock"))
        .err()
        .unwrap();
    assert!(message.contains("fs-groups serve"), "{message}");
}
