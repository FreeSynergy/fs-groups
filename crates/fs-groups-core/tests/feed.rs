//! The feed: every write of the repository is told, with the envelope the store committed.

mod common;

use std::sync::{Arc, Mutex};

use common::{person, repo};
use fs_groups_core::{GroupChange, GroupFeed, GroupListener, Member};
use fs_objects::ObjectId;

#[derive(Default)]
struct Seen(Mutex<Vec<GroupChange>>);

impl GroupListener for Seen {
    fn changed(&self, change: &GroupChange) {
        self.0.lock().unwrap().push(change.clone());
    }
}

impl Seen {
    fn take(&self) -> Vec<GroupChange> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

fn types(changes: &[GroupChange]) -> Vec<String> {
    changes
        .iter()
        .map(|c| match c {
            GroupChange::Created(e) => format!("created {}", e.object_type),
            GroupChange::Updated(e) => format!("updated {}", e.object_type),
            GroupChange::Deleted(_) => "deleted".to_string(),
        })
        .collect()
}

#[tokio::test]
async fn every_write_is_told_and_the_drop_ends_it() {
    let r = repo().await;
    let seen = Arc::new(Seen::default());
    let subscription = GroupFeed::subscribe(&r, seen.clone());

    let g = r.create("Team", "team").await.unwrap();
    let changes = seen.take();
    let GroupChange::Created(envelope) = &changes[0] else {
        panic!("{changes:?}");
    };
    assert_eq!(*envelope, g.envelope, "the envelope as stored");

    let renamed = r.rename(g.id, "Crew").await.unwrap();
    assert_eq!(seen.take(), [GroupChange::Updated(renamed.envelope)]);

    r.add(g.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    assert_eq!(types(&seen.take()), ["created Relationship"]);

    let deleted: Vec<ObjectId> = vec![g.id];
    r.delete(g.id).await.unwrap();
    let changes = seen.take();
    assert_eq!(
        changes.len(),
        2,
        "the membership and the group: {changes:?}"
    );
    assert_eq!(changes[1], GroupChange::Deleted(deleted[0]));

    drop(subscription);
    r.create("Unwatched", "").await.unwrap();
    assert_eq!(seen.take(), [] as [fs_groups_core::GroupChange; 0]);
}
