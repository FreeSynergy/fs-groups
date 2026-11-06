//! Changes of the provider to a subscribed consumer.
//!
//! Pattern: Observer + Adapter — the core calls its listener on the thread that made the change; this
//! one only puts the change into a channel. The shared forwarder sends it. The core's change already
//! carries the envelope it committed, so nothing has to be read again.

use std::sync::Arc;

use fs_groups_core::{GroupChange, GroupFeed, GroupListener};
use fs_interfaces::common_capnp::{change_listener, subscription};
use fs_interfaces::rpc::{ChangeSender, Notice, change_channel, forward};

struct ToChannel(ChangeSender<GroupChange>);

impl GroupListener for ToChannel {
    fn changed(&self, change: &GroupChange) {
        self.0.send(change.clone());
    }
}

/// Subscribe `listener` until the returned capability is dropped.
pub(crate) fn subscribe(
    feed: &Arc<dyn GroupFeed>,
    listener: change_listener::Client,
) -> subscription::Client {
    let (tx, rx) = change_channel();
    let core_subscription = feed.subscribe(Arc::new(ToChannel(tx)));
    forward(rx, core_subscription, listener, |change| async move {
        Some(notice(change))
    })
}

fn notice(change: GroupChange) -> Notice {
    match change {
        GroupChange::Created(envelope) => Notice::Created(envelope),
        GroupChange::Updated(envelope) => Notice::Updated(envelope),
        GroupChange::Deleted(id) => Notice::Deleted(id),
    }
}
