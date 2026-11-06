//! [`GroupRepo`]: the store-backed implementation of the groups traits.
//!
//! Pattern: Repository + Facade — one small type hides the object store and the envelope rules behind
//! `Groups`, `Memberships`, `Invitations`, `Delegations` and `Mirror`. Its collaborators are injected
//! as the narrow store traits `ObjectRepository` and `ObjectQuery`, so each can be swapped.
//!
//! **One write path.** The store handle `objects` is private to this file. The only way to put or
//! delete an object is a [`Writer`], which holds the write lock for as long as it lives. A method that
//! reads and then writes takes one `Writer` first and does both through it, so a method added later
//! cannot write without the lock: it does not compile.
//!
//! **Single-writer limit.** The lock is process wide and keyed by the identity of the store handle
//! given at construction, so every `GroupRepo` (clones and separately built ones) over the same
//! handle shares it. It does not cover a second process, nor a second handle opened on the same
//! database file; the store has no compare-and-swap to build on. Run one writer per database.

use std::collections::{BTreeSet, HashMap};
use std::ops::Deref;
use std::sync::{Arc, Mutex as StdMutex, OnceLock, PoisonError, Weak};

use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{ObjectId, Store, StoredObject};
use tokio::sync::{Mutex, MutexGuard};

use super::backend::StoreAdapter;
use crate::error::{Error, Result};
use crate::text;
use crate::traits::AiMarking;

/// Groups over an object store. Cheap to clone; clones share one write lock.
///
/// Writes that read first (nesting, renames, memberships, invitations, delegations, delete) run one
/// at a time per store handle in this process. **Single-writer limit:** that is all the safety there
/// is. Two processes, or two handles opened on one database file, can still overwrite each other (for
/// example store a cycle); keep to one writer per database. See `DESIGN.md`.
#[derive(Clone)]
pub struct GroupRepo {
    objects: Arc<dyn ObjectRepository>,
    pub(super) reads: Arc<dyn ObjectQuery>,
    pub(super) actor: String,
    pub(super) instance: String,
    pub(super) page_size: usize,
    /// The host's answer to "is this member an agent" (ADR 0056); none wired = no answer.
    pub(super) ai: Option<Arc<dyn AiMarking>>,
    /// Role ids that count as admin besides `Right::Manage` (configuration).
    pub(super) admin_roles: BTreeSet<String>,
    /// The identity of whoever installed: the top admin (configuration, provisional).
    pub(super) installer: Option<String>,
    lock: Arc<Mutex<()>>,
}

/// The locks of all live store handles of this process, by handle identity (an address that cannot be
/// reused while a `GroupRepo` holds the handle).
fn lock_for(key: usize) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<usize, Weak<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    map.retain(|_, w| w.strong_count() > 0);
    if let Some(live) = map.get(&key).and_then(Weak::upgrade) {
        return live;
    }
    let fresh = Arc::new(Mutex::new(()));
    map.insert(key, Arc::downgrade(&fresh));
    fresh
}

impl GroupRepo {
    /// Groups over a whole [`Store`]. `actor` is the IRI of the acting person / program (log).
    pub fn new(store: Arc<dyn Store>, actor: impl Into<String>) -> Self {
        let key = Arc::as_ptr(&store).cast::<()>().addr();
        let adapter = Arc::new(StoreAdapter(store));
        Self::build(adapter.clone(), adapter, actor.into(), key)
    }

    /// Groups over the narrow store traits only (a remote or fake backend can supply them). The write
    /// lock is shared with every other repository built over the same `objects` handle.
    pub fn with_backend(
        objects: Arc<dyn ObjectRepository>,
        reads: Arc<dyn ObjectQuery>,
        actor: impl Into<String>,
    ) -> Self {
        let key = Arc::as_ptr(&objects).cast::<()>().addr();
        Self::build(objects, reads, actor.into(), key)
    }

    fn build(
        objects: Arc<dyn ObjectRepository>,
        reads: Arc<dyn ObjectQuery>,
        actor: String,
        key: usize,
    ) -> Self {
        Self {
            objects,
            reads,
            actor,
            instance: "local.invalid".into(),
            page_size: 100,
            ai: None,
            admin_roles: BTreeSet::new(),
            installer: None,
            lock: lock_for(key),
        }
    }

    /// Instance domain for external IRIs `https://<instance>/groups/<id>` (ADR 0027).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Items per page of `list` (default 100, at least 1).
    #[must_use]
    pub fn with_page_size(mut self, n: usize) -> Self {
        self.page_size = n.max(1);
        self
    }

    /// Wire the host's answer to "is this member an agent". **Without it every admin grant is
    /// refused** (the safe reading of "no answer"), see [`AiMarking`].
    #[must_use]
    pub fn with_ai_marking(mut self, ai: Arc<dyn AiMarking>) -> Self {
        self.ai = Some(ai);
        self
    }

    /// Role ids that count as admin besides `Right::Manage`; an agent cannot hold them. Compared in
    /// folded form (trimmed, case folded), so `" Owner "` and `"owner"` are one role. Text with
    /// control or invisible characters, or an empty id, is refused.
    pub fn with_admin_roles<I, S>(mut self, roles: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut folded = BTreeSet::new();
        for role in roles {
            let role = role.as_ref().trim();
            text::check_role("admin role", role)?;
            let f = text::fold(role);
            if f.is_empty() {
                return Err(Error::Conflict("an admin role id is empty".into()));
            }
            folded.insert(f);
        }
        self.admin_roles = folded;
        Ok(self)
    }

    /// The identity of whoever installed: the top admin, who may do everything on every group that is
    /// not on a loop (unless marked as an agent). **Provisional, and not authentication:** this crate
    /// only compares the string it is asked about. Padded, empty or invisible text is refused.
    pub fn with_installer(mut self, installer: impl Into<String>) -> Result<Self> {
        let installer = installer.into();
        if installer.is_empty() || installer != installer.trim() {
            return Err(Error::Conflict(
                "the installer identity must not be empty or padded".into(),
            ));
        }
        self.installer = Some(text::person("installer", &installer)?);
        Ok(self)
    }

    /// The configured top admin, if any.
    pub fn installer(&self) -> Option<&str> {
        self.installer.as_deref()
    }

    pub(super) fn iri(&self, id: ObjectId) -> String {
        format!("https://{}/groups/{}", self.instance, id)
    }

    /// One stored object by id (a read; no lock).
    pub(super) async fn load(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self.objects.get(id).await?)
    }

    /// Take the write lock. Everything that writes goes through the returned [`Writer`].
    pub(super) async fn writer(&self) -> Writer<'_> {
        Writer {
            repo: self,
            _guard: self.lock.lock().await,
        }
    }
}

/// The write lock held, and the only way to put or delete a stored object. Reads (all methods of
/// [`GroupRepo`]) are available through `Deref` and see no other writer's half-done work.
pub(super) struct Writer<'a> {
    repo: &'a GroupRepo,
    _guard: MutexGuard<'a, ()>,
}

impl Deref for Writer<'_> {
    type Target = GroupRepo;
    fn deref(&self) -> &GroupRepo {
        self.repo
    }
}

impl Writer<'_> {
    /// Store an object (create or replace).
    pub(super) async fn put(&self, object: StoredObject) -> Result<()> {
        self.repo.objects.put(object, &self.repo.actor).await?;
        Ok(())
    }

    /// Remove an object.
    pub(super) async fn remove_object(&self, id: ObjectId) -> Result<()> {
        self.repo.objects.delete(id, &self.repo.actor).await?;
        Ok(())
    }
}
