//! Effective membership through nested groups, with the path.

mod common;

use common::{force_parent, person, repo, setup};
use fs_groups_core::Member;

#[tokio::test]
async fn a_member_of_a_child_belongs_to_the_parent_with_the_path() {
    let r = repo().await;
    let (dept, team) = (
        r.create("dept", "department").await.unwrap(),
        r.create("team", "team").await.unwrap(),
    );
    common::link(&r, team.id, dept.id).await;
    r.add(dept.id, Member::Person(person("boss")), "")
        .await
        .unwrap();
    r.add(team.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    let eff = r.effective(dept.id).await.unwrap();
    assert_eq!(eff.len(), 2);
    assert_eq!(
        (eff[0].person.as_str(), eff[0].via.clone()),
        (person("boss").as_str(), vec![dept.id])
    );
    assert_eq!(
        (eff[1].person.as_str(), eff[1].via.clone()),
        (person("ada").as_str(), vec![dept.id, team.id])
    );
    // the other way round nothing flows
    assert_eq!(r.effective(team.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn in_a_diamond_a_person_counts_once() {
    let r = repo().await;
    let mut g = vec![];
    for n in ["top", "left", "right", "bottom"] {
        g.push(r.create(n, "team").await.unwrap());
    }
    for (c, p) in [(1, 0), (2, 0), (3, 1), (3, 2)] {
        common::link(&r, g[c].id, g[p].id).await;
    }
    r.add(g[3].id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    let eff = r.effective(g[0].id).await.unwrap();
    assert_eq!(eff.len(), 1);
    assert_eq!(eff[0].via.len(), 3);
    assert_eq!(r.groups_of(&person("ada")).await.unwrap().len(), 4);
}

#[tokio::test]
async fn groups_of_and_is_member_follow_nesting_upward() {
    let r = repo().await;
    let (a, b, c) = (
        r.create("a", "t").await.unwrap(),
        r.create("b", "t").await.unwrap(),
        r.create("c", "t").await.unwrap(),
    );
    common::link(&r, b.id, a.id).await;
    r.add(b.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    assert!(r.is_member(a.id, &person("ada")).await.unwrap());
    assert!(r.is_member(b.id, &person("ada")).await.unwrap());
    assert!(!r.is_member(c.id, &person("ada")).await.unwrap());
    assert!(!r.is_member(a.id, &person("nobody")).await.unwrap());
    let mut want = vec![a.id, b.id];
    want.sort();
    assert_eq!(r.groups_of(&person("ada")).await.unwrap(), want);
}

#[tokio::test]
async fn effective_membership_ends_on_data_with_a_cycle() {
    let (r, store) = setup().await;
    let (a, b) = (
        r.create("a", "t").await.unwrap(),
        r.create("b", "t").await.unwrap(),
    );
    common::link(&r, b.id, a.id).await;
    force_parent(&store, a.id, b.id).await;
    r.add(a.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.add(b.id, Member::Person(person("bo")), "").await.unwrap();
    assert_eq!(r.effective(a.id).await.unwrap().len(), 2);
    assert_eq!(r.effective(b.id).await.unwrap().len(), 2);
    assert_eq!(r.groups_of(&person("ada")).await.unwrap().len(), 2);
}
