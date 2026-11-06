//! Shared helpers of the integration tests.

#![allow(dead_code)] // each test file uses a different subset

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use fs_groups_core::{DirectoryGroup, Error, GroupDirectory, GroupRepo, Result};
use fs_interfaces::Capabilities;
use fs_store::{ObjectId, Store, Timestamp, TursoStore};

pub const ME: &str = "https://local.invalid/users/owner";

/// A repository over a fresh in-memory store.
pub async fn repo() -> GroupRepo {
    setup().await.0
}

/// The repository and a handle to its store (to plant data the API would refuse).
pub async fn setup() -> (GroupRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    (
        GroupRepo::new(store.clone(), ME).with_instance("example.org"),
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
}

impl FakeDirectory {
    pub fn get(&self, id: ObjectId) -> Option<DirectoryGroup> {
        self.groups.lock().unwrap().get(&id).cloned()
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
    async fn list(&self) -> Result<Vec<DirectoryGroup>> {
        Ok(self.groups.lock().unwrap().values().cloned().collect())
    }
    async fn apply(&self, g: &DirectoryGroup) -> Result<()> {
        if *self.refuse.lock().unwrap() == Some(g.id) {
            return Err(Error::Directory("refused on purpose".into()));
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.groups.lock().unwrap().insert(g.id, g.clone());
        Ok(())
    }
    async fn remove(&self, id: ObjectId) -> Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.groups.lock().unwrap().remove(&id);
        Ok(())
    }
}
