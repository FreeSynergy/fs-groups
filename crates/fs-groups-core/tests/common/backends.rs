//! Test backends and directories that meddle: slow reads, injected faults, a recording directory.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fs_groups_core::{DirectoryGroup, Error, GroupDirectory, GroupRepo, Result};
use fs_interfaces::Capabilities;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Activity, LogEntry, ObjectId, Query, Store, StoredObject, TursoStore};

use super::{Agents, ME};

/// A backend that answers every read late, so a second writer gets in between a read and the write
/// that depends on it. This forces the interleavings a fast in-memory store hides.
pub struct Slow(pub Arc<dyn Store>);

async fn pause() {
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
}

#[async_trait]
impl ObjectRepository for Slow {
    async fn put(&self, o: StoredObject, actor: &str) -> fs_objects::Result<Activity> {
        Store::put(&*self.0, o, actor).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> fs_objects::Result<Vec<Activity>> {
        Store::put_and_log(&*self.0, o, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> fs_objects::Result<Option<StoredObject>> {
        let r = Store::get(&*self.0, id).await;
        pause().await;
        r
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> fs_objects::Result<Activity> {
        Store::delete(&*self.0, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Slow {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        let r = Store::query(&*self.0, q).await;
        pause().await;
        r
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        Store::count(&*self.0, q).await
    }
}

/// `n` repositories over one slow store (each its own instance, as two callers would have).
pub async fn slow_repos(n: usize) -> Vec<GroupRepo> {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let slow = Arc::new(Slow(store));
    (0..n)
        .map(|_| {
            GroupRepo::with_backend(slow.clone(), slow.clone(), ME)
                .with_instance("example.org")
                .with_ai_marking(Agents::with(&[]))
        })
        .collect()
}

/// Nest `child` under `parent`: ask, and approve as the parent's creator (the repository's actor).
pub async fn link(r: &GroupRepo, child: ObjectId, parent: ObjectId) -> fs_groups_core::Group {
    let request = r.request_parent(child, parent).await.unwrap();
    r.approve_parent(request.id).await.unwrap()
}

/// A backend that fails writes and queries it is told to, for the faults a real store has.
pub struct Faulty {
    pub inner: Arc<dyn Store>,
    pub fail_put: Box<dyn Fn(&StoredObject) -> bool + Send + Sync>,
    pub fail_query: Box<dyn Fn(&Query) -> bool + Send + Sync>,
}

fn injected() -> fs_objects::Error {
    fs_objects::Error::Invalid("injected fault".into())
}

#[async_trait]
impl ObjectRepository for Faulty {
    async fn put(&self, o: StoredObject, actor: &str) -> fs_objects::Result<Activity> {
        if (self.fail_put)(&o) {
            return Err(injected());
        }
        Store::put(&*self.inner, o, actor).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> fs_objects::Result<Vec<Activity>> {
        Store::put_and_log(&*self.inner, o, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> fs_objects::Result<Option<StoredObject>> {
        Store::get(&*self.inner, id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> fs_objects::Result<Activity> {
        Store::delete(&*self.inner, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Faulty {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        if (self.fail_query)(q) {
            return Err(injected());
        }
        Store::query(&*self.inner, q).await
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        Store::count(&*self.inner, q).await
    }
}

/// A repository over `store` whose writes / queries fail where the closures say so.
pub fn faulty_repo(
    store: &Arc<dyn Store>,
    fail_put: impl Fn(&StoredObject) -> bool + Send + Sync + 'static,
    fail_query: impl Fn(&Query) -> bool + Send + Sync + 'static,
) -> GroupRepo {
    let f = Arc::new(Faulty {
        inner: store.clone(),
        fail_put: Box::new(fail_put),
        fail_query: Box::new(fail_query),
    });
    GroupRepo::with_backend(f.clone(), f, ME)
        .with_instance("example.org")
        .with_ai_marking(Agents::with(&[]))
}

/// A directory that records its whole state after every call, and refuses the applies it is told to.
/// The intermediate states of a run can then be checked, not only the last one.
#[derive(Default)]
pub struct Recording {
    pub groups: Mutex<BTreeMap<ObjectId, DirectoryGroup>>,
    pub states: Mutex<Vec<BTreeMap<ObjectId, DirectoryGroup>>>,
    pub refuse: Mutex<std::collections::BTreeSet<ObjectId>>,
}

impl Recording {
    fn snapshot(&self) {
        let now = self.groups.lock().unwrap().clone();
        self.states.lock().unwrap().push(now);
    }
    /// The state before the run, then after each call of the run.
    pub fn start(&self) {
        self.states.lock().unwrap().clear();
        self.snapshot();
    }
}

#[async_trait]
impl GroupDirectory for Recording {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("recording");
        c.supported("directory.apply").supported("directory.read");
        c
    }
    async fn read(&self, id: ObjectId) -> Result<Option<DirectoryGroup>> {
        Ok(self.groups.lock().unwrap().get(&id).cloned())
    }
    async fn list(&self) -> Result<Vec<DirectoryGroup>> {
        Ok(self.groups.lock().unwrap().values().cloned().collect())
    }
    async fn apply(&self, g: &DirectoryGroup) -> Result<()> {
        let refused = self.refuse.lock().unwrap().contains(&g.id);
        if !refused {
            self.groups
                .lock()
                .unwrap()
                .insert(g.id, g.clone().normalised());
        }
        self.snapshot();
        if refused {
            Err(Error::Directory("refused".into()))
        } else {
            Ok(())
        }
    }
    async fn remove(&self, id: ObjectId) -> Result<()> {
        self.groups.lock().unwrap().remove(&id);
        self.snapshot();
        Ok(())
    }
}
