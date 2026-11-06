//! The right-flow setting of a group (ADR 0055 point 6; the meaning of up and down is provisional).

mod common;

use common::{person, repo, setup};
use fs_groups_core::{Group, GroupRepo, Groups, Member, Right, RightFlow};

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
async fn the_default_is_none_and_the_setting_is_kept() {
    let r = repo().await;
    let g = r.create("g", "t").await.unwrap();
    assert_eq!(g.right_flow, RightFlow::None);
    r.set_right_flow(g.id, RightFlow::Up).await.unwrap();
    assert_eq!(
        r.get(g.id).await.unwrap().unwrap().right_flow,
        RightFlow::Up
    );
    let again = Groups::set_right_flow(&r, g.id, RightFlow::Down)
        .await
        .unwrap();
    assert_eq!(again.right_flow, RightFlow::Down);
    assert!(
        r.set_right_flow(fs_store::ObjectId::new(), RightFlow::Up)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn an_old_object_without_the_field_reads_as_none() {
    let (r, store) = setup().await;
    let g = r.create("g", "t").await.unwrap();
    r.set_right_flow(g.id, RightFlow::Down).await.unwrap();
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("fs:rightFlow");
    store.put(o, common::ME).await.unwrap();
    assert_eq!(
        r.get(g.id).await.unwrap().unwrap().right_flow,
        RightFlow::None
    );
    let o = store.get(g.id).await.unwrap().unwrap();
    assert!(o.content.get("fs:rightFlow").is_none());
    r.set_right_flow(g.id, RightFlow::Up).await.unwrap();
    let o = store.get(g.id).await.unwrap().unwrap();
    assert_eq!(o.content["fs:rightFlow"], "up");
}

#[tokio::test]
async fn none_keeps_a_right_on_its_own_group() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    r.add_parent(c.id, p.id).await.unwrap();
    holds(&r, &p, "ada").await;
    assert!(may(&r, &p, "ada").await);
    assert!(!may(&r, &c, "ada").await);
    holds(&r, &c, "bo").await;
    assert!(!may(&r, &p, "bo").await);
}

#[tokio::test]
async fn down_reaches_children_onward_while_each_passes_down() {
    let r = repo().await;
    let top = mk(&r, "top", RightFlow::Down).await;
    let mid = mk(&r, "mid", RightFlow::Down).await;
    let low = mk(&r, "low", RightFlow::None).await;
    let beyond = mk(&r, "beyond", RightFlow::Down).await;
    r.add_parent(mid.id, top.id).await.unwrap();
    r.add_parent(low.id, mid.id).await.unwrap();
    r.add_parent(beyond.id, low.id).await.unwrap();
    holds(&r, &top, "ada").await;
    assert!(may(&r, &mid, "ada").await);
    assert!(
        may(&r, &low, "ada").await,
        "low is reached, it just does not go on"
    );
    assert!(!may(&r, &beyond, "ada").await, "low does not pass down");
    // and not upward
    holds(&r, &mid, "bo").await;
    assert!(!may(&r, &top, "bo").await);
}

#[tokio::test]
async fn the_group_the_right_passes_from_decides_not_the_one_it_reaches() {
    let r = repo().await;
    // a parent that does not pass, a child that would pass down: nothing moves
    let p = mk(&r, "p", RightFlow::None).await;
    let c = mk(&r, "c", RightFlow::Down).await;
    r.add_parent(c.id, p.id).await.unwrap();
    holds(&r, &p, "ada").await;
    assert!(!may(&r, &c, "ada").await);
    // a parent that passes down into a child that says none: the child is reached
    r.set_right_flow(p.id, RightFlow::Down).await.unwrap();
    r.set_right_flow(c.id, RightFlow::None).await.unwrap();
    assert!(may(&r, &c, "ada").await);
}

#[tokio::test]
async fn up_reaches_parents_onward_while_each_passes_up() {
    let r = repo().await;
    let top = mk(&r, "top", RightFlow::Up).await;
    let mid = mk(&r, "mid", RightFlow::Up).await;
    let low = mk(&r, "low", RightFlow::Up).await;
    r.add_parent(mid.id, top.id).await.unwrap();
    r.add_parent(low.id, mid.id).await.unwrap();
    holds(&r, &low, "ada").await;
    assert!(may(&r, &mid, "ada").await);
    assert!(may(&r, &top, "ada").await);
    r.set_right_flow(mid.id, RightFlow::None).await.unwrap();
    assert!(may(&r, &mid, "ada").await);
    assert!(!may(&r, &top, "ada").await, "mid does not pass up");
    holds(&r, &top, "bo").await;
    assert!(!may(&r, &low, "bo").await, "up does not go down");
}

#[tokio::test]
async fn a_diamond_is_walked_once_and_a_mixed_branch_stops_there() {
    let r = repo().await;
    let top = mk(&r, "top", RightFlow::Down).await;
    let left = mk(&r, "left", RightFlow::Down).await;
    let right = mk(&r, "right", RightFlow::None).await;
    let bottom = mk(&r, "bottom", RightFlow::None).await;
    for (c, p) in [
        (&left, &top),
        (&right, &top),
        (&bottom, &left),
        (&bottom, &right),
    ] {
        r.add_parent(c.id, p.id).await.unwrap();
    }
    holds(&r, &top, "ada").await;
    assert!(
        may(&r, &bottom, "ada").await,
        "through left, which passes down"
    );
    r.set_right_flow(left.id, RightFlow::None).await.unwrap();
    assert!(
        !may(&r, &bottom, "ada").await,
        "neither branch passes down any more"
    );
    assert!(may(&r, &left, "ada").await && may(&r, &right, "ada").await);
}

#[tokio::test]
async fn manage_still_implies_the_others_across_a_flow() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::Down).await,
        mk(&r, "c", RightFlow::None).await,
    );
    r.add_parent(c.id, p.id).await.unwrap();
    r.grant(p.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    assert!(
        r.may_do(c.id, &person("ada"), Right::RemoveMembers)
            .await
            .unwrap()
    );
}
