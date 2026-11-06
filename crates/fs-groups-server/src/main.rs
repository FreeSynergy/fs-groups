//! `fs-groups-server`. Usage: `fs-groups-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]`.
//!
//! `--actor` is only a name: the core decides "may approve" about it and nothing authenticates it.
//!
//! Pattern: none (thin binary) — reads its locations, opens the store, serves until a signal.

use std::path::PathBuf;
use std::sync::Arc;

use fs_groups_core::GroupRepo;
use fs_groups_server::GroupsServer;
use fs_interfaces::rpc::{default_socket, run_until_signal};
use fs_store::{Store, TursoStore};

const USAGE: &str = "fs-groups-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]\nDefaults: $XDG_RUNTIME_DIR/freesynergy/groups.sock, $XDG_DATA_HOME/freesynergy/groups, the local owner, local.invalid\n--actor is a name the program acts under, not a proof: nothing authenticates it.";
const LOCAL_OWNER: &str = "https://local.invalid/users/owner";
const LOCAL_INSTANCE: &str = "local.invalid";

fn default_data() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("freesynergy").join("groups"))
}

fn fail(message: &str) -> ! {
    eprintln!("fs-groups-server: {message}");
    std::process::exit(2);
}

fn main() {
    let mut socket = default_socket("groups");
    let mut data = default_data();
    let mut actor = LOCAL_OWNER.to_string();
    let mut instance = LOCAL_INSTANCE.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => socket = args.next().map(PathBuf::from),
            "--data" => data = args.next().map(PathBuf::from),
            "--actor" => actor = args.next().unwrap_or_else(|| fail("--actor needs a value")),
            "--instance" => {
                instance = args
                    .next()
                    .unwrap_or_else(|| fail("--instance needs a value"));
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => fail(&format!("unknown argument {other}")),
        }
    }
    let socket = socket.unwrap_or_else(|| fail("no $XDG_RUNTIME_DIR: pass --socket"));
    let data = data.unwrap_or_else(|| fail("no $XDG_DATA_HOME or $HOME: pass --data"));
    let served = run_until_signal(&socket, || async move {
        std::fs::create_dir_all(&data)?;
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let repo = Arc::new(GroupRepo::new(store, actor).with_instance(instance));
        Ok(GroupsServer::new(repo).into_client().client)
    });
    if let Err(e) = served {
        eprintln!("fs-groups-server: {e}");
        std::process::exit(1);
    }
}
