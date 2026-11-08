//! Mirroring into a directory, against an in-memory fake.

mod common;

use std::collections::BTreeSet;

use common::{FakeDirectory, person, repo};
use fs_groups_core::{DirectoryGroup, GroupDirectory, Groups, Member, Mirror};

async fn world() -> (
    fs_groups_core::GroupRepo,
    fs_groups_core::Group,
    fs_groups_core::Group,
) {
    let r = repo().await;
    let (dept, team) = (
        r.create("dept", "department").await.unwrap(),
        r.create("team", "team").await.unwrap(),
    );
    // both pass up: the edge is nested in the directory (option B, ADR 0056)
    for g in [&dept, &team] {
        r.set_right_flow(g.id, fs_groups_core::RightFlow::Up)
            .await
            .unwrap();
    }
    common::link(&r, team.id, dept.id).await;
    r.add(team.id, Member::Person(person("ada")), "member")
        .await
        .unwrap();
    r.add(team.id, Member::Person(person("ada")), "user")
        .await
        .unwrap(); // two roles, one person
    r.add(dept.id, Member::Person(person("boss")), "")
        .await
        .unwrap();
    (r, dept, team)
}

#[tokio::test]
async fn the_wanted_state_has_direct_people_and_direct_groups() {
    let (r, dept, team) = world().await;
    let want = r.desired().await.unwrap();
    assert_eq!(want[0].id, team.id, "children first");
    assert_eq!(
        want[0].members,
        [person("ada")],
        "a person once, roles are not mirrored"
    );
    assert_eq!(
        want[1],
        DirectoryGroup {
            id: dept.id,
            name: "dept".into(),
            members: vec![person("boss")],
            nested: vec![team.id]
        }
    );
}

#[tokio::test]
async fn the_first_sync_creates_everything_and_the_second_writes_nothing() {
    let (r, dept, team) = world().await;
    let dir = FakeDirectory::default();
    let first = r.sync_to(&dir).await.unwrap();
    assert_eq!(first.created, [team.id, dept.id]);
    assert_eq!(dir.len(), 2);
    let writes = dir.writes();
    let second = r.sync_to(&dir).await.unwrap();
    assert!(second.is_quiet());
    assert_eq!(second.unchanged.len(), 2);
    assert_eq!(dir.writes(), writes, "idempotent: nothing written");
}

#[tokio::test]
async fn changes_here_reach_the_directory_and_removals_follow() {
    let (r, dept, team) = world().await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    r.rename(team.id, "crew").await.unwrap();
    let m = r.direct(team.id).await.unwrap().remove(0);
    Groups::delete(&r, dept.id).await.unwrap();
    r.remove(m.id).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.updated, [team.id]);
    assert_eq!(report.removed, [dept.id]);
    assert_eq!(dir.get(team.id).unwrap().name, "crew");
    assert!(dir.get(dept.id).is_none());
    assert_eq!(
        dir.get(team.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
}

#[tokio::test]
async fn a_manual_change_in_the_directory_is_healed() {
    let (r, dept, team) = world().await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    {
        let mut groups = dir.groups.lock().unwrap();
        groups
            .get_mut(&team.id)
            .unwrap()
            .members
            .push("https://example.org/people/intruder".into());
        groups.get_mut(&dept.id).unwrap().members.clear();
    }
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.updated.len(), 2);
    assert_eq!(dir.get(team.id).unwrap().members, [person("ada")]);
    assert_eq!(dir.get(dept.id).unwrap().members, [person("boss")]);
}

#[tokio::test]
async fn a_group_deleted_in_the_directory_is_created_again() {
    let (r, _, team) = world().await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    dir.remove(team.id).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.created, [team.id]);
}

#[tokio::test]
async fn only_what_we_mirrored_is_ever_removed() {
    let (r, _, _) = world().await;
    let dir = FakeDirectory::default();
    let foreign = DirectoryGroup {
        id: fs_objects::ObjectId::new(),
        name: "not ours".into(),
        members: vec![],
        nested: vec![],
    };
    dir.apply(&foreign).await.unwrap();
    r.sync_to(&dir).await.unwrap();
    let again = r.sync_to(&dir).await.unwrap();
    assert_eq!(again.removed, [] as [fs_objects::ObjectId; 0]);
    assert!(
        dir.get(foreign.id).is_some(),
        "a group we never mirrored stays"
    );
}

#[tokio::test]
async fn the_ledger_survives_a_new_repository_over_the_same_store() {
    let (r, store) = common::setup().await;
    let g = r.create("solo", "team").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let again = fs_groups_core::GroupRepo::new(store, common::ME);
    Groups::delete(&again, g.id).await.unwrap();
    let report = again.sync_to(&dir).await.unwrap();
    assert_eq!(report.removed, [g.id]);
}

#[tokio::test]
async fn a_refused_group_is_reported_and_the_rest_goes_on() {
    let (r, dept, team) = world().await;
    let dir = FakeDirectory::default();
    *dir.refuse.lock().unwrap() = Some(team.id);
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, team.id);
    assert_eq!(report.created, [dept.id]);
    *dir.refuse.lock().unwrap() = None;
    let retry = r.sync_to(&dir).await.unwrap();
    assert_eq!(retry.created, [team.id]);
    assert_eq!(
        r.mirrored("fake").await.unwrap(),
        BTreeSet::from([team.id, dept.id])
    );
}
