//! `fs-groups-server`. Usage: `fs-groups-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]`.
//!
//! `--actor` is only a name: the core decides "may approve" about it and nothing authenticates it.
//!
//! Pattern: none (thin binary) — reads its locations, opens the store, serves until a signal.

use std::sync::Arc;

use fs_groups_core::GroupRepo;
use fs_groups_server::GroupsServer;
use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal};
use fs_objects::{Store, TursoStore};

fn main() {
    let options = ServerOptions::from_env_or_exit("groups");
    let served = run_until_signal(&options.socket, || async {
        prepare_data_dir(&options.data)?;
        let store = TursoStore::open(options.data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let repo = GroupRepo::new(store, options.actor.clone()).with_ai_marking(Arc::new(
            fs_groups_server::OperatorIsHuman(options.actor.clone()),
        ));
        let repo = match &options.instance {
            Some(instance) => repo.with_instance(instance),
            None => repo,
        };
        let repo = Arc::new(repo);
        Ok(GroupsServer::new(repo).into_client().client)
    });
    if let Err(e) = served {
        eprintln!("fs-groups-server: {e}");
        std::process::exit(1);
    }
}
