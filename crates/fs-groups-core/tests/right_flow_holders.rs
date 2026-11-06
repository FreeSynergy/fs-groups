//! Holder groups and bad data under the right-flow setting.

mod common;

use common::{force_parent, person, repo, setup};
use fs_groups_core::{Group, GroupRepo, Member, Right, RightFlow};

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn may(r: &GroupRepo, on: &Group, who: &str) -> bool {
    r.may_do(on.id, &person(who), Right::Read).await.unwrap()
}

async fn holds(r: &GroupRepo, on: &Group, who: &str) {
    r.grant(on.id, Member::Person(person(who)), Right::Read)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_holder_groups_right_reaches_its_direct_members_only_whatever_anyone_set() {
    let r = repo().await;
    let club = mk(&r, "club", RightFlow::None).await;
    let board = mk(&r, "board", RightFlow::Down).await;
    let sub = mk(&r, "sub", RightFlow::Down).await;
    r.add(board.id, Member::Person(person("boss")), "")
        .await
        .unwrap();
    r.add(sub.id, Member::Person(person("intern")), "")
        .await
        .unwrap();
    common::link(&r, sub.id, board.id).await;
    r.grant(club.id, Member::Group(board.id), Right::Manage)
        .await
        .unwrap();
    assert!(may(&r, &club, "boss").await);
    assert!(!may(&r, &club, "intern").await, "in a child of the board");
    // membership itself is about belonging and did not change
    assert!(r.is_member(board.id, &person("intern")).await.unwrap());
}

#[tokio::test]
async fn a_new_parent_does_not_capture_the_rights_of_a_holder() {
    let r = repo().await;
    let club = mk(&r, "club", RightFlow::None).await;
    let team = mk(&r, "team", RightFlow::Up).await;
    let dept = mk(&r, "dept", RightFlow::Up).await;
    r.add(dept.id, Member::Person(person("head")), "")
        .await
        .unwrap();
    r.grant(club.id, Member::Group(team.id), Right::Read)
        .await
        .unwrap();
    common::link(&r, team.id, dept.id).await;
    assert!(!may(&r, &club, "head").await);
}

#[tokio::test]
async fn cyclic_bad_data_never_grants_and_never_hangs() {
    let (r, store) = setup().await;
    let a = mk(&r, "a", RightFlow::Down).await;
    let b = mk(&r, "b", RightFlow::Down).await;
    common::link(&r, b.id, a.id).await;
    force_parent(&store, a.id, b.id).await;
    holds(&r, &a, "ada").await;
    assert!(!may(&r, &b, "ada").await);
    assert!(!may(&r, &a, "ada").await, "a is on the loop itself");
    r.add(b.id, Member::Person(person("bo")), "").await.unwrap();
    r.grant(a.id, Member::Group(a.id), Right::Read)
        .await
        .unwrap();
    assert!(!may(&r, &a, "bo").await);
}
