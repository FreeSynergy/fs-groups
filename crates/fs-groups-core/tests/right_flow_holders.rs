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
async fn a_holder_group_reaches_members_of_the_groups_its_flow_reaches() {
    let r = repo().await;
    let club = mk(&r, "club", RightFlow::None).await;
    let board = mk(&r, "board", RightFlow::Down).await;
    let sub = mk(&r, "sub", RightFlow::None).await;
    let subsub = mk(&r, "subsub", RightFlow::None).await;
    r.add_parent(sub.id, board.id).await.unwrap();
    r.add_parent(subsub.id, sub.id).await.unwrap();
    for (g, who) in [(&board, "boss"), (&sub, "mid"), (&subsub, "deep")] {
        r.add(g.id, Member::Person(person(who)), "").await.unwrap();
    }
    r.grant(club.id, Member::Group(board.id), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &club, "boss").await);
    assert!(may(&r, &club, "mid").await);
    assert!(!may(&r, &club, "deep").await, "sub does not pass down");
    // membership itself is about belonging and did not change
    assert!(r.is_member(board.id, &person("deep")).await.unwrap());
    // a holder group that passes up reaches the members of its parents
    let team = mk(&r, "team", RightFlow::Up).await;
    let dept = mk(&r, "dept", RightFlow::None).await;
    r.add_parent(team.id, dept.id).await.unwrap();
    r.add(dept.id, Member::Person(person("head")), "")
        .await
        .unwrap();
    r.grant(club.id, Member::Group(team.id), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &club, "head").await);
}

#[tokio::test]
async fn walks_end_on_cyclic_bad_data() {
    let (r, store) = setup().await;
    let a = mk(&r, "a", RightFlow::Down).await;
    let b = mk(&r, "b", RightFlow::Down).await;
    r.add_parent(b.id, a.id).await.unwrap();
    force_parent(&store, a.id, b.id).await;
    holds(&r, &a, "ada").await;
    assert!(may(&r, &b, "ada").await);
    r.add(b.id, Member::Person(person("bo")), "").await.unwrap();
    r.grant(a.id, Member::Group(a.id), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &a, "bo").await);
    r.set_right_flow(a.id, RightFlow::Up).await.unwrap();
    r.set_right_flow(b.id, RightFlow::Up).await.unwrap();
    assert!(may(&r, &a, "ada").await);
}
