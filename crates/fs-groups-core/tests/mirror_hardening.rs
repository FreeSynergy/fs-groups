//! The mirror under faults: unreadable groups, a ledger that fails, a list that leaves one out, two
//! directories of one provider, a refused child, a person the directory cannot map.

mod common;

use common::{FakeDirectory, ME, faulty_repo, person, repo, setup};
use fs_groups_core::{Member, Mirror};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn a_group_without_a_kind_is_still_a_group() {
    let (r, store) = setup().await;
    let g = r.create("g", "team").await.unwrap();
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("fs:kind");
    store.put(o, ME).await.unwrap();
    assert_eq!(r.get(g.id).await.unwrap().unwrap().kind, "");
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).is_some());
    r.sync_to(&dir).await.unwrap();
    assert!(
        dir.get(g.id).is_some(),
        "and is not removed by the next run"
    );
}

#[tokio::test]
async fn an_unreadable_group_is_reported_and_never_removed() {
    let (r, store) = setup().await;
    let g = r.create("g", "team").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("name");
    store.put(o, ME).await.unwrap();
    assert!(
        r.get(g.id).await.unwrap().is_none(),
        "cannot be read as a group"
    );
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.unreadable, [g.id]);
    assert!(report.removed.is_empty());
    assert!(dir.get(g.id).is_some());
    // and once readable again it is still ours
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.content["name"] = "g".into();
    store.put(o, ME).await.unwrap();
    r.delete(g.id).await.unwrap();
    assert_eq!(r.sync_to(&dir).await.unwrap().removed, [g.id]);
}

#[tokio::test]
async fn a_failing_ledger_before_the_run_changes_nothing_in_the_directory() {
    let (r, store) = setup().await;
    r.create("g", "team").await.unwrap();
    let broken = faulty_repo(
        &store,
        |o| o.envelope.object_type == "MirrorLedger",
        |_| false,
    );
    let dir = FakeDirectory::default();
    assert!(broken.sync_to(&dir).await.is_err());
    assert_eq!(
        dir.len(),
        0,
        "the intent could not be written, so nothing was applied"
    );
}

#[tokio::test]
async fn a_ledger_that_fails_after_the_run_loses_nothing() {
    let (r, store) = setup().await;
    let puts = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = puts.clone();
    // the first ledger write (the intent) works, later ones fail
    let flaky = faulty_repo(
        &store,
        move |o| {
            o.envelope.object_type == "MirrorLedger" && counter.fetch_add(1, Ordering::SeqCst) >= 1
        },
        |_| false,
    );
    let g = flaky.create("g", "team").await.unwrap();
    let dir = FakeDirectory::default();
    let report = flaky.sync_to(&dir).await.unwrap();
    assert!(report.ledger_error.is_some());
    assert_eq!(report.created, [g.id], "the report is not lost");
    // the group is gone here; the ledger still knows it was put there
    r.delete(g.id).await.unwrap();
    assert_eq!(r.sync_to(&dir).await.unwrap().removed, [g.id]);
    assert_eq!(dir.len(), 0);
}

#[tokio::test]
async fn a_group_the_list_left_out_once_is_not_forgotten() {
    let r = repo().await;
    let g = r.create("g", "team").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    r.delete(g.id).await.unwrap();
    dir.hide.lock().unwrap().push(g.id);
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.removed, [g.id], "found by asking for it directly");
    assert_eq!(dir.len(), 0);
}

#[tokio::test]
async fn a_group_the_directory_no_longer_has_leaves_the_ledger() {
    let r = repo().await;
    let g = r.create("g", "team").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    r.delete(g.id).await.unwrap();
    dir.groups.lock().unwrap().clear();
    r.sync_to(&dir).await.unwrap();
    assert!(
        r.mirrored(&dir.directory_id_for_test())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn two_directories_of_one_provider_have_their_own_ledger() {
    let r = repo().await;
    let g = r.create("g", "team").await.unwrap();
    let (one, two) = (FakeDirectory::default(), FakeDirectory::default());
    *one.identity.lock().unwrap() = Some("https://idm-one.example.org".into());
    *two.identity.lock().unwrap() = Some("https://idm-two.example.org".into());
    r.sync_to(&one).await.unwrap();
    r.sync_to(&two).await.unwrap();
    r.delete(g.id).await.unwrap();
    assert_eq!(r.sync_to(&one).await.unwrap().removed, [g.id]);
    assert_eq!(r.sync_to(&two).await.unwrap().removed, [g.id]);
    assert_eq!((one.len(), two.len()), (0, 0));
}

#[tokio::test]
async fn a_refused_child_is_not_left_nested_in_its_parent() {
    let r = repo().await;
    let (p, c) = (
        r.create("p", "t").await.unwrap(),
        r.create("c", "t").await.unwrap(),
    );
    for g in [&p, &c] {
        r.set_right_flow(g.id, fs_groups_core::RightFlow::Up).await.unwrap();
    }
    common::link(&r, c.id, p.id).await;
    let dir = FakeDirectory::default();
    *dir.refuse.lock().unwrap() = Some(c.id);
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.unnested, [(p.id, c.id)]);
    assert!(dir.get(p.id).unwrap().nested.is_empty());
    // the refusal ends: the next run nests it
    *dir.refuse.lock().unwrap() = None;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(report.unnested.is_empty());
    assert_eq!(dir.get(p.id).unwrap().nested, [c.id]);
}

#[tokio::test]
async fn one_unmapped_person_does_not_stop_the_rest_of_the_group() {
    let r = repo().await;
    let g = r.create("g", "t").await.unwrap();
    for who in ["ada", "bo"] {
        r.add(g.id, Member::Person(person(who)), "").await.unwrap();
    }
    let dir = FakeDirectory::default();
    *dir.unmapped.lock().unwrap() = Some(person("bo"));
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.created, [g.id]);
    assert_eq!(report.unmapped, [(g.id, vec![person("bo")])]);
    assert!(report.failed.is_empty());
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
    // ada leaves: the removal is applied although bo still cannot be mapped
    let m = r.direct(g.id).await.unwrap().remove(0);
    r.remove(m.id).await.unwrap();
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).unwrap().members.is_empty());
}
