//! `GroupsRemote`: the proxy and the worker thread that owns the connection.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection
//! and every call is a job handed to it; the caller waits for the answer on a plain channel. Strategy
//! (`Dial`): a server that is there (plain), or the version handshake of fs-interfaces (a server of
//! another version is replaced, none is started by the starter). Every call has a time limit
//! ([`call_limited`]): a server that does not answer is reported, the window never freezes.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use fs_interfaces::groups_capnp::{groups, items, memberships};
use fs_interfaces::rpc::client::{CALL_TIMEOUT, Fault, call_limited};
use fs_interfaces::rpc::{Connection, Expected, ServerStarter, connect, connect_matching};

use super::model::ApiError;

/// The name of the server in the words of a timeout.
const SERVER: &str = "groups";

/// What the caller waits on top of a limit for a worker that cannot run the job.
const GRACE: Duration = Duration::from_secs(2);

/// How long the window waits for the server before it says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The questions asked right after the handshake: the window is not open yet, so short.
    pub start: Duration,
    /// Every other call: the default of fs-interfaces (`CALL_TIMEOUT`, 30 s).
    pub call: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            start: Duration::from_secs(10),
            call: CALL_TIMEOUT,
        }
    }
}

pub(super) struct Clients {
    pub(super) items: items::Client,
    pub(super) memberships: memberships::Client,
}

type Job = Box<dyn FnOnce(Rc<Clients>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// How the connection is made.
enum Dial {
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
    fn open(&self, limits: Limits) -> Result<tokio::sync::mpsc::UnboundedSender<Job>, String> {
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

/// The state of the connection: a worker that answers, or why there is none.
enum Link {
    Up(tokio::sync::mpsc::UnboundedSender<Job>),
    Down(String),
}

/// The groups server on a socket.
pub struct GroupsRemote {
    dial: Dial,
    limits: Limits,
    link: Mutex<Link>,
}

impl GroupsRemote {
    /// Connect to the server on `socket`: no version check, nothing is started (what the tests need).
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect(socket: &Path) -> Result<Self, String> {
        Self::connect_with(socket, Limits::default())
    }

    /// [`GroupsRemote::connect`] with other [`Limits`].
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect_with(socket: &Path, limits: Limits) -> Result<Self, String> {
        let dial = Dial::Plain(socket.to_path_buf());
        let jobs = dial.open(limits)?;
        Ok(Self {
            dial,
            limits,
            link: Mutex::new(Link::Up(jobs)),
        })
    }

    /// Reach the server on `socket` that fits this program (the version handshake of fs-interfaces): one that
    /// runs and differs is replaced, none running is started by `starter`. This never fails: when no fitting
    /// server can be reached, the remote keeps the reason, every call answers with it and tries again first
    /// (so refreshing the window is the "Try again"). The window opens in any case.
    pub fn matching(socket: &Path, starter: Arc<dyn ServerStarter + Send + Sync>) -> Self {
        Self::matching_with(socket, starter, Limits::default())
    }

    /// [`GroupsRemote::matching`] with other [`Limits`].
    pub fn matching_with(
        socket: &Path,
        starter: Arc<dyn ServerStarter + Send + Sync>,
        limits: Limits,
    ) -> Self {
        let dial = Dial::Matching {
            socket: socket.to_path_buf(),
            expected: Expected::new(env!("CARGO_PKG_VERSION")),
            starter,
        };
        let link = match dial.open(limits) {
            Ok(jobs) => Link::Up(jobs),
            Err(e) => Link::Down(e),
        };
        Self {
            dial,
            limits,
            link: Mutex::new(link),
        }
    }

    /// Connect again (after a failed handshake) and keep the new connection.
    ///
    /// # Errors
    /// The text for the person.
    pub fn reconnect(&self) -> Result<(), String> {
        let opened = self.dial.open(self.limits);
        let mut link = self.link.lock().unwrap_or_else(PoisonError::into_inner);
        match opened {
            Ok(jobs) => {
                *link = Link::Up(jobs);
                Ok(())
            }
            Err(e) => {
                *link = Link::Down(e.clone());
                Err(e)
            }
        }
    }

    pub(super) fn run<T, F, Fut>(&self, make: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, ApiError>> + 'static,
    {
        let limit = self.limits.call;
        let down = matches!(
            &*self.link.lock().unwrap_or_else(PoisonError::into_inner),
            Link::Down(_)
        );
        if down {
            self.reconnect().map_err(ApiError::Failed)?;
        }
        let jobs = match &*self.link.lock().unwrap_or_else(PoisonError::into_inner) {
            Link::Up(jobs) => jobs.clone(),
            Link::Down(e) => return Err(ApiError::Failed(e.clone())),
        };
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |clients| {
            Box::pin(async move {
                let done = call_limited(SERVER, limit, make(clients))
                    .await
                    .unwrap_or_else(|fault| Err(timed_out(&fault)));
                let _ = reply.send(done);
            })
        });
        jobs.send(job).map_err(|_| gone())?;
        // The job ends itself at `limit`; this is the belt for a worker that does not run it at all.
        match answer.recv_timeout(limit + GRACE) {
            Ok(done) => done,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(timed_out(&Fault::TimedOut {
                server: SERVER,
                limit,
            })),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(gone()),
        }
    }
}

pub(super) fn gone() -> ApiError {
    ApiError::Failed("the groups server went away".into())
}

fn timed_out(fault: &Fault) -> ApiError {
    ApiError::Failed(fault.to_string())
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
