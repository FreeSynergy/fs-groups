//! A group becomes a child only once the parent's side approves (provisional: who consents is still
//! the owner's to answer). Not enforced: the actor is whatever string the repository was built with.

mod common;

use common::{ME, person, repo, setup};
use fs_groups_core::{Error, GroupRepo, Member, Mirror, RequestState, Right};

const OTHER: &str = "https://example.org/users/other";

#[tokio::test]
async fn provisional_asking_links_nothing_and_approving_does() {
    let r = repo().await;
    let (c, p) = (
        r.create("c", "t").await.unwrap(),
        r.create("p", "t").await.unwrap(),
    );
    let req = r.request_parent(c.id, p.id).await.unwrap();
    assert_eq!(req.state, RequestState::Pending);
    assert!(r.parents(c.id).await.unwrap().is_empty());
    assert!(r.children(p.id).await.unwrap().is_empty());
    assert!(r.get(c.id).await.unwrap().unwrap().parents.is_empty());
    let g = r.approve_parent(req.id).await.unwrap();
    assert_eq!(g.parents, [p.id]);
    assert_eq!(r.parents(c.id).await.unwrap(), [p.id]);
    let listed = r.parent_requests(p.id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, RequestState::Approved);
    assert_eq!(listed[0].requester, ME);
    assert!(
        matches!(r.approve_parent(req.id).await, Err(Error::Conflict(_))),
        "answered once"
    );
}

#[tokio::test]
async fn declining_links_nothing_and_asking_again_is_possible() {
    let r = repo().await;
    let (c, p) = (
        r.create("c", "t").await.unwrap(),
        r.create("p", "t").await.unwrap(),
    );
    let req = r.request_parent(c.id, p.id).await.unwrap();
    assert_eq!(
        r.request_parent(c.id, p.id).await.unwrap().id,
        req.id,
        "a pending request is returned, not doubled"
    );
    r.decline_parent(req.id).await.unwrap();
    assert!(r.parents(c.id).await.unwrap().is_empty());
    assert!(r.approve_parent(req.id).await.is_err());
    let second = r.request_parent(c.id, p.id).await.unwrap();
    assert_ne!(second.id, req.id);
    assert_eq!(r.parent_requests(p.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn provisional_only_the_parents_side_may_answer() {
    let (r, store) = setup().await;
    let (c, p) = (
        r.create("c", "t").await.unwrap(),
        r.create("p", "t").await.unwrap(),
    );
    let stranger = GroupRepo::new(store.clone(), OTHER)
        .with_instance("example.org")
        .with_ai_marking(common::Agents::with(&[]));
    let req = stranger.request_parent(c.id, p.id).await.unwrap();
    assert_eq!(req.requester, OTHER, "anybody may ask");
    // no delegation on p yet: only its creator may answer
    assert!(matches!(
        stranger.approve_parent(req.id).await,
        Err(Error::Forbidden(_))
    ));
    assert!(matches!(
        stranger.decline_parent(req.id).await,
        Err(Error::Forbidden(_))
    ));
    // a delegation on p ends the creator's special standing
    r.grant(p.id, Member::Person(person("boss")), Right::Manage)
        .await
        .unwrap();
    assert!(matches!(
        r.approve_parent(req.id).await,
        Err(Error::Forbidden(_))
    ));
    // whoever holds manage on p may answer
    r.grant(p.id, Member::Person(OTHER.into()), Right::Manage)
        .await
        .unwrap();
    stranger.approve_parent(req.id).await.unwrap();
    assert_eq!(r.parents(c.id).await.unwrap(), [p.id]);
}

#[tokio::test]
async fn a_loop_is_refused_when_asking_and_again_when_approving() {
    let r = repo().await;
    let (x, y) = (
        r.create("x", "t").await.unwrap(),
        r.create("y", "t").await.unwrap(),
    );
    let xy = r.request_parent(x.id, y.id).await.unwrap();
    // y under x is fine to ask for as long as x has no parent: the loop only closes on approval
    let yx = r.request_parent(y.id, x.id).await.unwrap();
    r.approve_parent(xy.id).await.unwrap();
    assert!(matches!(
        r.approve_parent(yx.id).await,
        Err(Error::Cycle { .. })
    ));
    assert!(r.parents(y.id).await.unwrap().is_empty());
    assert!(matches!(
        r.request_parent(y.id, x.id).await,
        Err(Error::Cycle { .. })
    ));
    assert!(matches!(
        r.request_parent(x.id, x.id).await,
        Err(Error::Cycle { .. })
    ));
}

#[tokio::test]
async fn the_mirror_sees_approved_links_only() {
    let r = repo().await;
    let (c, p, q) = (
        r.create("c", "t").await.unwrap(),
        r.create("p", "t").await.unwrap(),
        r.create("q", "t").await.unwrap(),
    );
    for g in [&c, &p] {
        r.set_right_flow(g.id, fs_groups_core::RightFlow::Up)
            .await
            .unwrap();
    }
    common::link(&r, c.id, p.id).await;
    r.request_parent(c.id, q.id).await.unwrap();
    let desired = r.desired().await.unwrap();
    let nested = |id| desired.iter().find(|g| g.id == id).unwrap().nested.clone();
    assert_eq!(nested(p.id), [c.id]);
    assert!(nested(q.id).is_empty());
}

#[tokio::test]
async fn deleting_a_group_removes_its_requests() {
    let r = repo().await;
    let (c, p) = (
        r.create("c", "t").await.unwrap(),
        r.create("p", "t").await.unwrap(),
    );
    let req = r.request_parent(c.id, p.id).await.unwrap();
    r.delete(c.id).await.unwrap();
    assert!(r.parent_requests(p.id).await.unwrap().is_empty());
    assert!(r.approve_parent(req.id).await.is_err());
}
