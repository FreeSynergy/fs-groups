//! The window over a real server: the standard menus (ADR 0065) with a view per data type of the catalog.

use std::sync::Arc;

use fs_groups_app::GroupsRemote;
use fs_groups_app::gui::app_widget;
use fs_groups_core::GroupRepo;
use fs_groups_core::describe::TYPES;
use fs_groups_server::{GroupsServer, OperatorIsHuman};
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};

#[test]
fn the_window_has_a_screen_per_data_type_and_the_standard_menus() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
    let path = socket.clone();
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let me = "https://local.invalid/users/owner";
            let repo =
                GroupRepo::new(store, me).with_ai_marking(Arc::new(OperatorIsHuman::new(me)));
            let root = GroupsServer::new(Arc::new(repo)).into_client();
            let bound = bind_socket(&path).await.unwrap();
            up.send(()).unwrap();
            serve(bound, root.client, std::future::pending()).await.ok();
        });
    });
    started.recv().unwrap();
    let remote = Arc::new(GroupsRemote::connect(&socket).unwrap());
    let app = app_widget(&remote).unwrap();
    app.with_shell(|s| {
        let ids: Vec<&str> = s
            .menus()
            .iter()
            .flat_map(|m| m.entries.iter().map(|e| e.id.as_str()))
            .collect();
        for t in TYPES {
            assert!(ids.contains(&format!("view-{t}").as_str()), "{t}");
        }
        assert!(ids.contains(&"help-about") && ids.contains(&"program-quit"));
    });
}
