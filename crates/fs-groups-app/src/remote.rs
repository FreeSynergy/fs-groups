//! `GroupsRemote`: the groups server behind a socket, as a [`GroupsApi`].
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection
//! and every call is a job handed to it; the caller waits for the answer on a plain channel (the
//! window thread is not inside a runtime). [`GroupsApi`] is the trait first: the adapter knows only it.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use fs_interfaces::common_capnp;
use fs_interfaces::groups_capnp::{group, groups, items, memberships};
use fs_interfaces::rpc::{Problem, connect, read_envelope, read_uuid, write_uuid};
use uuid::Uuid;

/// A group as the window needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The id.
    pub id: String,
    /// The name.
    pub name: String,
    /// What kind of group: free text.
    pub kind: String,
    /// The ids of the groups it belongs to.
    pub parents: Vec<String>,
}

/// Who is in a group, directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// A person (an IRI) or, for a group, its id.
    pub who: String,
    /// Whether `who` is a group.
    pub is_group: bool,
    /// The role id; empty for a plain member.
    pub role: String,
}

/// Why a call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// There is no such group.
    NotFound(String),
    /// The server cannot do this (the provider rule).
    Unsupported(String),
    /// The server refused or failed; the text is its own.
    Failed(String),
}

/// What the window asks of the groups program.
pub trait GroupsApi: Send + Sync {
    /// All groups, in the server's order.
    ///
    /// # Errors
    /// [`ApiError`].
    fn list(&self) -> Result<Vec<Group>, ApiError>;
    /// One group, `None` if there is none.
    ///
    /// # Errors
    /// [`ApiError`].
    fn get(&self, id: &str) -> Result<Option<Group>, ApiError>;
    /// A new group.
    ///
    /// # Errors
    /// [`ApiError`].
    fn create(&self, name: &str, kind: &str) -> Result<Group, ApiError>;
    /// Change the name.
    ///
    /// # Errors
    /// [`ApiError`].
    fn rename(&self, id: &str, name: &str) -> Result<Group, ApiError>;
    /// Change the kind.
    ///
    /// # Errors
    /// [`ApiError`].
    fn set_kind(&self, id: &str, kind: &str) -> Result<Group, ApiError>;
    /// Delete the group, with its memberships.
    ///
    /// # Errors
    /// [`ApiError`].
    fn delete(&self, id: &str) -> Result<(), ApiError>;
    /// The direct members.
    ///
    /// # Errors
    /// [`ApiError`].
    fn members(&self, id: &str) -> Result<Vec<Member>, ApiError>;
}

struct Clients {
    items: items::Client,
    memberships: memberships::Client,
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

    fn run<T, F, Fut>(&self, make: F) -> Result<T, ApiError>
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

fn gone() -> ApiError {
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

fn problem(error: &capnp::Error) -> ApiError {
    match Problem::from_error(error) {
        Some(p) if p.is_unsupported() => ApiError::Unsupported(p.detail),
        Some(p) if p.status == 404 => ApiError::NotFound(p.detail),
        Some(p) => ApiError::Failed(p.detail),
        None => ApiError::Failed(error.to_string()),
    }
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> {
    id.parse()
        .map_err(|_| ApiError::NotFound(format!("\"{id}\" names no group.")))
}

fn read_group(g: group::Reader<'_>) -> capnp::Result<Group> {
    let envelope = read_envelope(g.get_envelope()?)?;
    let parents = g
        .get_parents()?
        .iter()
        .filter_map(read_uuid)
        .map(|u| u.to_string())
        .collect();
    Ok(Group {
        id: envelope.id.to_string(),
        name: envelope.name,
        kind: g.get_kind()?.to_string()?,
        parents,
    })
}

impl GroupsApi for GroupsRemote {
    fn list(&self) -> Result<Vec<Group>, ApiError> {
        self.run(|c| async move {
            let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
            loop {
                let mut request = c.items.list_request();
                request.get().init_cursor().set_opaque(&cursor);
                let answer = request.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(items::list_results::Reader::get_page)
                    .map_err(|e| problem(&e))?;
                for g in page.get_items().map_err(|e| problem(&e))? {
                    found.push(read_group(g).map_err(|e| problem(&e))?);
                }
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    return Ok(found);
                }
                cursor = next.to_vec();
            }
        })
    }

    fn get(&self, id: &str) -> Result<Option<Group>, ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let mut request = c.items.get_request();
            write_uuid(request.get().init_id(), Some(id));
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            let r = answer.get().map_err(|e| problem(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            let g = r
                .get_group()
                .and_then(read_group)
                .map_err(|e| problem(&e))?;
            Ok(Some(g))
        })
    }

    fn create(&self, name: &str, kind: &str) -> Result<Group, ApiError> {
        let (name, kind) = (name.to_string(), kind.to_string());
        self.run(move |c| async move {
            let mut request = c.items.create_request();
            request.get().set_name(name.as_str());
            request.get().set_kind(kind.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::create_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn rename(&self, id: &str, name: &str) -> Result<Group, ApiError> {
        let (id, name) = (parse_id(id)?, name.to_string());
        self.run(move |c| async move {
            let mut request = c.items.rename_request();
            write_uuid(request.get().init_id(), Some(id));
            request.get().set_name(name.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::rename_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn set_kind(&self, id: &str, kind: &str) -> Result<Group, ApiError> {
        let (id, kind) = (parse_id(id)?, kind.to_string());
        self.run(move |c| async move {
            let mut request = c.items.set_kind_request();
            write_uuid(request.get().init_id(), Some(id));
            request.get().set_kind(kind.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::set_kind_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn delete(&self, id: &str) -> Result<(), ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let mut request = c.items.delete_request();
            write_uuid(request.get().init_id(), Some(id));
            request
                .send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    fn members(&self, id: &str) -> Result<Vec<Member>, ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
            loop {
                let mut request = c.memberships.direct_request();
                write_uuid(request.get().init_group(), Some(id));
                request.get().init_cursor().set_opaque(&cursor);
                let answer = request.send().promise.await.map_err(|e| problem(&e))?;
                let r = answer.get().map_err(|e| problem(&e))?;
                for m in r.get_items().map_err(|e| problem(&e))? {
                    let who = m.get_member().map_err(|e| problem(&e))?;
                    let (who, is_group) =
                        match who.which().map_err(|e| ApiError::Failed(e.to_string()))? {
                            fs_interfaces::groups_capnp::member::Which::Person(p) => (
                                p.map_err(|e| problem(&e))?
                                    .to_string()
                                    .map_err(|e| ApiError::Failed(e.to_string()))?,
                                false,
                            ),
                            fs_interfaces::groups_capnp::member::Which::Group(g) => (
                                read_uuid(g.map_err(|e| problem(&e))?)
                                    .map(|u| u.to_string())
                                    .unwrap_or_default(),
                                true,
                            ),
                        };
                    found.push(Member {
                        who,
                        is_group,
                        role: m
                            .get_role()
                            .and_then(|t| t.to_string().map_err(Into::into))
                            .map_err(|e: capnp::Error| problem(&e))?,
                    });
                }
                let next = r
                    .get_next()
                    .map_err(|e| problem(&e))?
                    .get_opaque()
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    return Ok(found);
                }
                cursor = next.to_vec();
            }
        })
    }
}
