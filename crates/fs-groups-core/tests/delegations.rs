//! Delegation: finer than all-or-nothing, and the question `may_do`.

mod common;

use common::{person, repo};
use fs_groups_core::{Delegations, Error, Member, Right, RightFlow};

#[tokio::test]
async fn a_direct_right_answers_only_itself() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::AddMembers)
        .await
        .unwrap();
    assert!(
        r.may_do(g.id, &person("ada"), Right::AddMembers)
            .await
            .unwrap()
    );
    assert!(
        !r.may_do(g.id, &person("ada"), Right::RemoveMembers)
            .await
            .unwrap()
    );
    assert!(!r.may_do(g.id, &person("ada"), Right::Read).await.unwrap());
    assert!(
        !r.may_do(g.id, &person("bo"), Right::AddMembers)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn manage_implies_the_others() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    for right in [
        Right::Read,
        Right::AddMembers,
        Right::RemoveMembers,
        Right::Manage,
    ] {
        assert!(
            r.may_do(g.id, &person("ada"), right).await.unwrap(),
            "{right:?}"
        );
    }
}

#[tokio::test]
async fn a_right_held_by_a_group_counts_for_its_direct_members_whatever_its_flow() {
    let r = repo().await;
    let (club, board, inner) = (
        r.create("club", "t").await.unwrap(),
        r.create("board", "t").await.unwrap(),
        r.create("inner", "t").await.unwrap(),
    );
    common::link(&r, inner.id, board.id).await;
    r.add(board.id, Member::Person(person("boss")), "")
        .await
        .unwrap();
    r.add(inner.id, Member::Person(person("deep")), "")
        .await
        .unwrap();
    r.grant(club.id, Member::Group(board.id), Right::RemoveMembers)
        .await
        .unwrap();
    assert!(
        r.may_do(club.id, &person("boss"), Right::RemoveMembers)
            .await
            .unwrap()
    );
    // ADR 0055 point 6: by default a holder group's right reaches its direct members only
    assert!(
        !r.may_do(club.id, &person("deep"), Right::RemoveMembers)
            .await
            .unwrap()
    );
    // safe reading (provisional): the board's flow does not carry the right to members of children
    r.set_right_flow(board.id, RightFlow::Down).await.unwrap();
    assert!(
        !r.may_do(club.id, &person("deep"), Right::RemoveMembers)
            .await
            .unwrap()
    );
    assert!(
        !r.may_do(club.id, &person("outsider"), Right::RemoveMembers)
            .await
            .unwrap()
    );
    assert!(
        !r.may_do(club.id, &person("boss"), Right::AddMembers)
            .await
            .unwrap()
    );
    // leaving the group ends the right
    let m = r.direct(board.id).await.unwrap().remove(0);
    r.remove(m.id).await.unwrap();
    assert!(
        !r.may_do(club.id, &person("boss"), Right::RemoveMembers)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn granting_twice_returns_the_first_and_revoking_ends_it() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let d = r
        .grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    let again = r
        .grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    assert_eq!(d.id, again.id);
    assert_eq!(Delegations::list(&r, g.id).await.unwrap().len(), 1);
    Delegations::revoke(&r, d.id).await.unwrap();
    assert!(!r.may_do(g.id, &person("ada"), Right::Read).await.unwrap());
    assert!(matches!(
        Delegations::revoke(&r, d.id).await,
        Err(Error::NotFound { .. })
    ));
}

#[tokio::test]
async fn a_right_on_a_parent_does_not_reach_the_child_by_default() {
    let r = repo().await;
    let (p, c) = (
        r.create("p", "t").await.unwrap(),
        r.create("c", "t").await.unwrap(),
    );
    common::link(&r, c.id, p.id).await;
    r.grant(p.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    assert!(!r.may_do(c.id, &person("ada"), Right::Read).await.unwrap());
}
