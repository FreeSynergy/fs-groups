//! Shared helpers of the integration tests: a real server on a real Unix socket with a real store, and the backend
//! the window uses.

#![allow(dead_code)] // each test file uses a different subset

use std::path::Path;
use std::sync::Arc;

use fs_admin_frontend::Values;
use fs_groups_app::{GroupsBackend, GroupsRemote};
use fs_groups_core::GroupRepo;
use fs_groups_server::{GroupsServer, OperatorIsHuman};
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};

pub const ME: &str = "https://local.invalid/users/owner";
pub const ALICE: &str = "https://local.invalid/users/alice";

pub fn start_server(socket: &Path) {
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

pub fn values(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect()))
        .collect()
}

pub struct World {
    pub _dir: tempfile::TempDir,
    pub remote: Arc<GroupsRemote>,
}

impl World {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("groups.sock");
        start_server(&socket);
        let remote = Arc::new(GroupsRemote::connect(&socket).unwrap());
        Self { _dir: dir, remote }
    }

    pub fn of(&self, data_type: &'static str) -> GroupsBackend {
        GroupsBackend::new(Arc::clone(&self.remote), data_type)
    }
}

pub fn id_of(o: fs_admin_frontend::Outcome) -> String {
    o.id.expect("the command names the record it made")
}
