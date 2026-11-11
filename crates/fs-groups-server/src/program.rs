//! `serve`: the server as a command of the one program (ADR 0063).
//!
//! Pattern: Facade — reads its locations, opens the store, serves until a signal. `fs-groups serve`
//! calls it. `--actor` is only a name: the core decides "may approve" about it and nothing
//! authenticates it.

use std::ffi::OsString;
use std::sync::Arc;

use fs_groups_core::GroupRepo;
use fs_interfaces::rpc::{ServerIdentity, ServerOptions, prepare_data_dir, run_until_signal_as};
use fs_objects::{Store, TursoStore};

use crate::{GroupsServer, OperatorIsHuman};

/// The help text of `fs-groups serve`.
pub const HELP: &str = "fs-groups serve [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN] [--facet groups]\nDefaults: $XDG_RUNTIME_DIR/freesynergy/groups.sock, $XDG_DATA_HOME/freesynergy/groups, the local owner";

/// Why `serve` ended without serving.
#[derive(Debug, PartialEq, Eq)]
pub enum ServeError {
    /// The words were wrong; the caller should exit with 2.
    Usage(String),
    /// The server could not start or stopped with an error; exit with 1.
    Failed(String),
}

/// Serve `groups.capnp` until a signal arrives. `args` are the words after `serve`.
///
/// # Errors
/// [`ServeError::Usage`] for unknown or incomplete words, [`ServeError::Failed`] for everything else.
pub fn serve(args: impl IntoIterator<Item = OsString>) -> Result<(), ServeError> {
    let Some(options) = ServerOptions::parse("groups", args).map_err(ServeError::Usage)? else {
        println!("{HELP}");
        return Ok(());
    };
    // The one facet of this program is `groups`; a name it does not have is refused, so a wrong link is found at the start.
    if let Some(facet) = &options.facet
        && facet != "groups"
    {
        return Err(ServeError::Usage(format!(
            "unknown facet {facet}; this program has: groups"
        )));
    }
    // Name and version go to `Control.version`: a window of another version replaces this server (fs-interfaces handshake).
    let identity = ServerIdentity::new("fs-groups", env!("CARGO_PKG_VERSION"));
    run_until_signal_as(&options.socket, identity, || async {
        prepare_data_dir(&options.data)?;
        let store = TursoStore::open(options.data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let repo = GroupRepo::new(store, options.actor.clone())
            .with_ai_marking(Arc::new(OperatorIsHuman::new(&options.actor)));
        let repo = match &options.instance {
            Some(instance) => repo.with_instance(instance),
            None => repo,
        };
        let repo = Arc::new(repo);
        Ok(GroupsServer::new(repo).into_client().client)
    })
    .map_err(|e| ServeError::Failed(e.to_string()))
}
