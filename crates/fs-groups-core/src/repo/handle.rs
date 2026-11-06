//! [`GroupRepo`]: the store-backed implementation of the groups traits.
//!
//! Pattern: Repository + Facade — one small type hides the object store and the envelope rules behind
//! `Groups`, `Memberships`, `Invitations`, `Delegations` and `Mirror`. Its collaborators are injected
//! as the narrow store traits `ObjectRepository` and `ObjectQuery`, so each can be swapped.

use std::sync::Arc;

use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{ObjectId, Store};
use tokio::sync::Mutex;

use super::backend::StoreAdapter;

/// Groups over an object store. Cheap to clone; clones share one write lock.
#[derive(Clone)]
pub struct GroupRepo {
    pub(super) objects: Arc<dyn ObjectRepository>,
    pub(super) reads: Arc<dyn ObjectQuery>,
    pub(super) actor: String,
    pub(super) instance: String,
    pub(super) page_size: usize,
    /// Serialises the writes that read first (nesting, delete), so two cannot form a cycle together.
    pub(super) lock: Arc<Mutex<()>>,
}

impl GroupRepo {
    /// Groups over a whole [`Store`]. `actor` is the IRI of the acting person / program (log).
    pub fn new(store: Arc<dyn Store>, actor: impl Into<String>) -> Self {
        let adapter = Arc::new(StoreAdapter(store));
        Self::with_backend(adapter.clone(), adapter, actor)
    }

    /// Groups over the narrow store traits only (a remote or fake backend can supply them).
    pub fn with_backend(
        objects: Arc<dyn ObjectRepository>,
        reads: Arc<dyn ObjectQuery>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            objects,
            reads,
            actor: actor.into(),
            instance: "local.invalid".into(),
            page_size: 100,
            lock: Arc::new(Mutex::new(())),
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

    pub(super) fn iri(&self, id: ObjectId) -> String {
        format!("https://{}/groups/{}", self.instance, id)
    }
}
