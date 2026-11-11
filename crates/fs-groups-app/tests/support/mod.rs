//! Test support: a real server on its own thread that says it is `fs-groups` of a version.
//!
//! Pattern: none (tests).
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use fs_groups_core::GroupRepo;
use fs_groups_server::{GroupsServer, OperatorIsHuman};
use fs_interfaces::rpc::{ServerIdentity, run_until_signal_as};
use fs_objects::{Store, TursoStore};

const ME: &str = "https://local.invalid/users/owner";

/// A server that says it is `fs-groups` of `version` (`Control.version`), the way `fs-groups serve` does; it ends with
/// the process. Returns when it listens.
pub fn serve_as(socket: &Path, version: &str) {
    let (socket, version) = (socket.to_path_buf(), version.to_owned());
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = run_until_signal_as(
            &socket,
            ServerIdentity::new("fs-groups", version),
            || async move {
                let store: Arc<dyn Store> = Arc::new(
                    TursoStore::open_in_memory()
                        .await
                        .map_err(std::io::Error::other)?,
                );
                let repo =
                    GroupRepo::new(store, ME).with_ai_marking(Arc::new(OperatorIsHuman::new(ME)));
                up.send(()).ok();
                Ok(GroupsServer::new(Arc::new(repo)).into_client().client)
            },
        );
    });
    started.recv().expect("the server started");
}
