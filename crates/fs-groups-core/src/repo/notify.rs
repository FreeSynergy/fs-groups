//! Every committed write of the repository, told to the feed.
//!
//! Pattern: Decorator — wraps the `ObjectRepository` the repository writes through, so memberships,
//! cascades and state changes need no call of their own and none can be forgotten. The envelope comes
//! from the store's own after-image, so it is what was really committed. The mirror ledger is internal
//! bookkeeping and stays out; it is never deleted, so `Delete` needs no such check.

use std::sync::Arc;

use async_trait::async_trait;
use fs_objects::traits::ObjectRepository;
use fs_objects::{Activity, ActivityKind, LogEntry, ObjectId, Result, StoredObject};

use super::consts::LEDGER_TYPE;
use crate::traits::{GroupChange, Registry};

pub(super) struct Notifying {
    pub(super) inner: Arc<dyn ObjectRepository>,
    pub(super) feed: Arc<Registry>,
}

impl Notifying {
    fn tell(&self, activity: &Activity) {
        if self.feed.is_empty() {
            return;
        }
        let change = match activity.kind {
            ActivityKind::Delete => Some(GroupChange::Deleted(activity.object)),
            ActivityKind::Create => after(activity).map(GroupChange::Created),
            ActivityKind::Update => after(activity).map(GroupChange::Updated),
            _ => None,
        };
        if let Some(change) = change {
            self.feed.notify(&change);
        }
    }
}

fn after(activity: &Activity) -> Option<fs_objects::Envelope> {
    let image = activity.change_set().ok()?.after?;
    (image.envelope.object_type != LEDGER_TYPE).then_some(image.envelope)
}

#[async_trait]
impl ObjectRepository for Notifying {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        let activity = self.inner.put(object, actor).await?;
        self.tell(&activity);
        Ok(activity)
    }

    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        let activities = self.inner.put_and_log(object, actor, extra).await?;
        for a in &activities {
            self.tell(a);
        }
        Ok(activities)
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.inner.get(id).await
    }

    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        let activity = self.inner.delete(id, actor).await?;
        self.tell(&activity);
        Ok(activity)
    }
}
