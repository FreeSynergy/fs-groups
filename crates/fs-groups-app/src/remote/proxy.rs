//! `GroupsRemote`: the proxy and the worker thread that owns the connection.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection
//! and every call is a job handed to it; the caller waits for the answer on a plain channel.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use fs_interfaces::groups_capnp::{groups, items, memberships};
use fs_interfaces::rpc::connect;

use super::model::ApiError;

pub(super) struct Clients {
    pub(super) items: items::Client,
    pub(super) memberships: memberships::Client,
}

type Job = Box<dyn FnOnce(Rc<Clients>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// The groups server on a socket.
pub struct GroupsRemote {
    jobs: tokio::sync::mpsc::UnboundedSender<Job>,
}

impl GroupsRemote {
    /// Connect to the server on `socket`.
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect(socket: &Path) -> Result<Self, String> {
        let (jobs, inbox) = tokio::sync::mpsc::unbounded_channel();
        let (ready, started) = mpsc::channel();
        let socket = socket.to_path_buf();
        thread::Builder::new()
            .name("groups-rpc".into())
            .spawn(move || worker(&socket, inbox, &ready))
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
        Ok(Self { jobs })
    }

    pub(super) fn run<T, F, Fut>(&self, make: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, ApiError>> + 'static,
    {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |clients| {
            Box::pin(async move {
                let _ = reply.send(make(clients).await);
            })
        });
        self.jobs.send(job).map_err(|_| gone())?;
        answer.recv().map_err(|_| gone())?
    }
}

pub(super) fn gone() -> ApiError {
    ApiError::Failed("the groups server went away".into())
}

fn worker(
    socket: &Path,
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
        let connection = match connect::<groups::Client>(socket).await {
            Ok(c) => c,
            Err(e) => {
                let _ = ready.send(Err(format!(
                    "no groups server on {}: {e}; start it with `fs-groups serve`",
                    socket.display()
                )));
                return;
            }
        };
        let clients = match open(&connection.client).await {
            Ok(c) => Rc::new(c),
            Err(e) => {
                let _ = ready.send(Err(e.to_string()));
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
