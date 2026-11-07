//! Text shapes, canonical spelling and the minor findings of the third review.

mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{Agents, FakeDirectory, ME, person, setup};
use fs_groups_core::{AiMarking, Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_objects::Store;

async fn world(agents: &[&str]) -> (GroupRepo, Arc<dyn Store>, Arc<Agents>) {
    let (r, store) = setup().await;
    let list = Agents::with(agents);
    let r = r
        .with_ai_marking(list.clone())
        .with_admin_roles(["owner"])
        .unwrap();
    (r, store, list)
}

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

#[tokio::test]
async fn invisible_and_look_alike_text_is_refused_by_shape() {
    let (r, _, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    for bad in [
        "ada\u{2800}",
        "ada\u{8e2}",
        "ada\u{13430}",
        "ada\u{1bca0}",
        "ada\u{110bd}",
        "ada\u{1d173}",
        "ada smith",
        "ada\u{a0}x",
    ] {
        assert!(
            r.add(g.id, Member::Person(bad.into()), "").await.is_err(),
            "person {bad:?}"
        );
        assert!(
            r.add(g.id, Member::Person(person("ada")), bad)
                .await
                .is_err(),
            "role {bad:?}"
        );
    }
    // a Cyrillic o in an admin role
    assert!(
        r.add(g.id, Member::Person(person("ada")), "\u{43e}wner")
            .await
            .is_err()
    );
    assert!(
        r.add(g.id, Member::Person(person("ada")), "Owner")
            .await
            .is_ok()
    );
    assert!(
        r.add(
            g.id,
            Member::Person("https://example.org/people/Zoë".into()),
            ""
        )
        .await
        .is_ok()
    );
}

/// Records what it is asked.
struct Recorder(Mutex<Vec<String>>);

#[async_trait]
impl AiMarking for Recorder {
    async fn is_ai(&self, person: &str) -> fs_groups_core::Result<Option<bool>> {
        self.0.lock().unwrap().push(person.to_string());
        Ok(Some(false))
    }
}

#[tokio::test]
async fn the_host_is_asked_with_scheme_and_host_lowercased_and_nothing_else_changed() {
    let (r, _) = setup().await;
    let rec = Arc::new(Recorder(Mutex::new(vec![])));
    let r = r.with_ai_marking(rec.clone());
    let g = r.create("g", "t").await.unwrap();
    for id in [
        "HTTPS://ADA@IDM.Example.ORG:8443/Ada/?u=Ada#Frag",
        "https://example.org/people/ada/",
    ] {
        r.grant(g.id, Member::Person(id.into()), Right::Manage)
            .await
            .unwrap();
    }
    let asked: BTreeSet<_> = rec.0.lock().unwrap().iter().cloned().collect();
    assert_eq!(
        asked,
        [
            "https://ADA@idm.example.org:8443/Ada/?u=Ada#Frag".to_string(),
            "https://example.org/people/ada/".to_string()
        ]
        .into()
    );
}

// ---- minor ----

struct Broken;

#[async_trait]
impl AiMarking for Broken {
    async fn is_ai(&self, _: &str) -> fs_groups_core::Result<Option<bool>> {
        Err(fs_groups_core::Error::Directory("host down".into()))
    }
}

#[tokio::test]
async fn reads_answer_while_the_checker_fails_and_show_not_in_force() {
    let (r, _, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    r.add(g.id, Member::Person(person("ada")), "owner")
        .await
        .unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    let down = r.clone().with_ai_marking(Arc::new(Broken));
    assert!(!down.direct(g.id).await.unwrap()[0].in_force);
    assert!(!down.delegations(g.id).await.unwrap()[0].in_force);
    assert_eq!(down.effective(g.id).await.unwrap().len(), 1);
    down.add(g.id, Member::Person(person("bo")), "")
        .await
        .unwrap();
    down.grant(g.id, Member::Person(person("bo")), Right::Read)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_group_whose_type_changed_in_the_store_is_not_removed_from_the_directory() {
    let (r, store, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.envelope.object_type = "Note".into();
    store.put(o, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).is_some());
    assert_eq!(report.unreadable, [g.id]);
    assert!(!report.is_quiet());
}
