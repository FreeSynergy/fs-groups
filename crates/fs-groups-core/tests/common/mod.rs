//! Shared helpers of the integration tests.

#![allow(dead_code)] // each test file uses a different subset

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use fs_groups_core::{AiMarking, DirectoryGroup, Error, GroupDirectory, GroupRepo, Result};
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;
use fs_objects::{Store, Timestamp, TursoStore};

pub const ME: &str = "https://local.invalid/users/owner";

/// The host's answer to "is this member an agent": the listed ones are, everybody else is a human.
#[derive(Default)]
pub struct Agents(pub Mutex<Vec<String>>);

impl Agents {
    pub fn with(list: &[&str]) -> Arc<Self> {
        Arc::new(Self(Mutex::new(
            list.iter().map(|s| (*s).to_string()).collect(),
        )))
    }
}

#[async_trait]
impl AiMarking for Agents {
    async fn is_ai(&self, person: &str) -> Result<Option<bool>> {
        Ok(Some(self.0.lock().unwrap().iter().any(|a| a == person)))
    }
}

/// An answer that is not available.
pub struct NoAnswer;

#[async_trait]
impl AiMarking for NoAnswer {
    async fn is_ai(&self, _person: &str) -> Result<Option<bool>> {
        Ok(None)
    }
}

/// A repository over a fresh in-memory store.
pub async fn repo() -> GroupRepo {
    setup().await.0
}

/// The repository and a handle to its store (to plant data the API would refuse).
pub async fn setup() -> (GroupRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    (
        GroupRepo::new(store.clone(), ME)
            .with_instance("example.org")
            .with_ai_marking(Agents::with(&[])),
        store,
    )
}

/// The IRI of person number `n`.
pub fn person(n: &str) -> String {
    format!("https://example.org/people/{n}")
}

/// A point in time, `s` seconds after the epoch.
pub fn at(s: i64) -> Timestamp {
    Timestamp::from_timestamp(s, 0).unwrap()
}

/// Plant a parent link without the cycle check, as bad data from outside could.
pub async fn force_parent(store: &Arc<dyn Store>, child: ObjectId, parent: ObjectId) {
    let mut o = store.get(child).await.unwrap().unwrap();
    o.content["fs:parents"]
        .as_array_mut()
        .unwrap()
        .push(parent.to_string().into());
    o = o.with_index("parent", parent.to_string());
    store.put(o, ME).await.unwrap();
}

/// A directory kept in memory, with hooks to meddle with it.
#[derive(Default)]
pub struct FakeDirectory {
    pub groups: Mutex<BTreeMap<ObjectId, DirectoryGroup>>,
    pub writes: AtomicUsize,
    pub refuse: Mutex<Option<ObjectId>>,
    /// Groups `list` leaves out although the directory holds them.
    pub hide: Mutex<Vec<ObjectId>>,
    /// A person the directory cannot map: applied without them, then `Unmapped`.
    pub unmapped: Mutex<Option<String>>,
    /// The directory identity; `None` = the default (the provider name).
    pub identity: Mutex<Option<String>>,
}

impl FakeDirectory {
    pub fn get(&self, id: ObjectId) -> Option<DirectoryGroup> {
        self.groups.lock().unwrap().get(&id).cloned()
    }
    pub fn directory_id_for_test(&self) -> String {
        GroupDirectory::directory_id(self)
    }
    pub fn len(&self) -> usize {
        self.groups.lock().unwrap().len()
    }
    pub fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl GroupDirectory for FakeDirectory {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("fake");
        c.supported("directory.apply").supported("directory.read");
        c
    }
    async fn read(&self, id: ObjectId) -> Result<Option<DirectoryGroup>> {
        Ok(self.get(id))
    }
    fn directory_id(&self) -> String {
        self.identity
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.capabilities().provider().to_string())
    }
    async fn list(&self) -> Result<Vec<DirectoryGroup>> {
        let hide = self.hide.lock().unwrap().clone();
        Ok(self
            .groups
            .lock()
            .unwrap()
            .values()
            .filter(|g| !hide.contains(&g.id))
            .cloned()
            .collect())
    }
    async fn apply(&self, g: &DirectoryGroup) -> Result<()> {
        if *self.refuse.lock().unwrap() == Some(g.id) {
            return Err(Error::Directory("refused on purpose".into()));
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        let mut stored = g.clone();
        let skipped = self.unmapped.lock().unwrap().clone();
        let left_out = skipped.filter(|p| stored.members.contains(p));
        if let Some(p) = &left_out {
            stored.members.retain(|m| m != p);
        }
        self.groups.lock().unwrap().insert(g.id, stored);
        match left_out {
            Some(p) => Err(Error::Unmapped(vec![p])),
            None => Ok(()),
        }
    }
    async fn remove(&self, id: ObjectId) -> Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.groups.lock().unwrap().remove(&id);
        Ok(())
    }
}

mod backends;
#[allow(unused_imports)]
pub use backends::*;
