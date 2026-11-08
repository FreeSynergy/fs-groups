//! The [`GroupFeed`] contract: live changes of everything this program stores.
//!
//! Pattern: Observer — a [`GroupListener`] subscribes and is told about every change; dropping the
//! [`Subscription`] ends it (as in `common.capnp`). [`Registry`] is the subject; the store decorator
//! `repo/notify.rs` feeds it, so no write path can forget to tell.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_objects::{Envelope, ObjectId};

/// What happened to a group, membership, invitation or delegation; `envelope.object_type` tells which.
#[derive(Clone, Debug, PartialEq)]
pub enum GroupChange {
    /// An object was created.
    Created(Envelope),
    /// An object was changed.
    Updated(Envelope),
    /// An object was deleted.
    Deleted(ObjectId),
}

/// Receives changes. Called on the thread that made the change: keep it short, hand work off.
pub trait GroupListener: Send + Sync {
    /// Something changed.
    fn changed(&self, change: &GroupChange);
}

/// `group.subscribe`: live updates. Interface status: draft (ADR 0045).
pub trait GroupFeed: Send + Sync {
    /// Tell `listener` about every change until the returned [`Subscription`] is dropped.
    fn subscribe(&self, listener: Arc<dyn GroupListener>) -> Subscription;
}

type Entries = Vec<(u64, Arc<dyn GroupListener>)>;

/// The subject of the Observer: holds the listeners and notifies them.
#[derive(Default)]
pub struct Registry {
    next: Mutex<u64>,
    listeners: Mutex<Entries>,
}

impl Registry {
    /// Add a listener; the subscription removes it again.
    pub fn add(self: &Arc<Self>, listener: Arc<dyn GroupListener>) -> Subscription {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        let id = *next;
        self.entries().push((id, listener));
        Subscription {
            id,
            registry: Arc::clone(self),
        }
    }

    /// Tell every listener.
    pub fn notify(&self, change: &GroupChange) {
        let listeners: Vec<_> = self.entries().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l.changed(change);
        }
    }

    /// Whether anyone listens at all.
    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    fn entries(&self) -> MutexGuard<'_, Entries> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps a subscription alive; dropping it unsubscribes.
#[must_use = "dropping the subscription ends it"]
pub struct Subscription {
    id: u64,
    registry: Arc<Registry>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let id = self.id;
        self.registry.entries().retain(|(i, _)| *i != id);
    }
}
