//! The worker thread that owns the connection, and how the connection is made.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::thread;

use fs_interfaces::groups_capnp::{groups, items, memberships};
use fs_interfaces::rpc::client::call_limited;
use fs_interfaces::rpc::{Connection, Expected, ServerStarter, connect, connect_matching};

use super::{Limits, SERVER};

pub(in crate::remote) struct Clients {
    pub(in crate::remote) items: items::Client,
    pub(in crate::remote) memberships: memberships::Client,
}

pub(super) type Job = Box<dyn FnOnce(Rc<Clients>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// How the connection is made.
pub(super) enum Dial {
    Plain(PathBuf),
    Matching {
        socket: PathBuf,
        expected: Expected,
        starter: Arc<dyn ServerStarter + Send + Sync>,
    },
}

impl Dial {
    fn same(&self) -> Self {
        match self {
            Self::Plain(socket) => Self::Plain(socket.clone()),
            Self::Matching {
                socket,
                expected,
                starter,
            } => Self::Matching {
                socket: socket.clone(),
                expected: expected.clone(),
                starter: Arc::clone(starter),
            },
        }
    }

    /// Start a worker thread that connects and then answers the jobs; wait until it is connected.
    pub(super) fn open(
        &self,
        limits: Limits,
    ) -> Result<tokio::sync::mpsc::UnboundedSender<Job>, String> {
        let (jobs, inbox) = tokio::sync::mpsc::unbounded_channel();
        let (ready, started) = mpsc::channel();
        let how = self.same();
        thread::Builder::new()
            .name("groups-rpc".into())
            .spawn(move || worker(&how, limits, inbox, &ready))
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
        Ok(jobs)
    }

    async fn connect(&self) -> Result<Connection<groups::Client>, String> {
        match self {
            Self::Plain(socket) => connect::<groups::Client>(socket).await.map_err(|e| {
                format!(
                    "no groups server on {}: {e}; start it with `fs-groups serve`",
                    socket.display()
                )
            }),
            Self::Matching {
                socket,
                expected,
                starter,
            } => connect_matching::<groups::Client>(socket, expected, starter.as_ref())
                .await
                .map_err(|e| e.to_string()),
        }
    }
}

fn worker(
    dial: &Dial,
    limits: Limits,
    mut inbox: tokio::sync::mpsc::UnboundedReceiver<Job>,
    ready: &mpsc::Sender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    tokio::task::LocalSet::new().block_on(&runtime, async move {
        let connection = match dial.connect().await {
            Ok(c) => c,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        // The first questions have a limit too: a server that passed the handshake and then says nothing must not
        // keep the window from opening.
        let clients = match call_limited(SERVER, limits.start, open(&connection.client)).await {
            Ok(Ok(c)) => Rc::new(c),
            Ok(Err(e)) => {
                let _ = ready.send(Err(e.to_string()));
                return;
            }
            Err(fault) => {
                let _ = ready.send(Err(fault.to_string()));
                return;
            }
        };
        let _ = ready.send(Ok(()));
        while let Some(job) = inbox.recv().await {
            tokio::task::spawn_local(job(Rc::clone(&clients)));
        }
    });
}

async fn open(root: &groups::Client) -> capnp::Result<Clients> {
    let items = root
        .items_request()
        .send()
        .promise
        .await?
        .get()?
        .get_items()?;
    let memberships = root
        .memberships_request()
        .send()
        .promise
        .await?
        .get()?
        .get_memberships()?;
    Ok(Clients { items, memberships })
}
