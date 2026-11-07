//! Following leaves "belongs to" alone, and tidies up when a group goes or a source is lost.
//! Tests against the `Follows` trait (provisional, ADR 0056).

mod common;

use common::{admin, group_iri, group_with, link, site};
use fs_groups_core::{DeliveryState, Error, Follows, Member, Right, SourceState};
use fs_objects::Query;

#[tokio::test]
async fn following_does_not_touch_parents_or_cycles() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let top = a.create("Top", "team").await.unwrap();
    let sub = a.create("Sub", "team").await.unwrap();
    link(&a, sub.id, top.id).await;
    for g in [top.id, sub.id] {
        a.grant(g, Member::Person(admin("a.example")), Right::Read)
            .await
            .unwrap();
    }
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let before = (
        a.parents(sub.id).await.unwrap(),
        a.children(top.id).await.unwrap(),
    );
    // mutual following of the same two groups: the child of this site and the group of the other
    let (_, to_b) = Follows::deliver(
        &a,
        sub.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let (_, to_sub) = Follows::deliver(
        &b,
        gb.id,
        &group_iri("a.example", sub.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let on_b = Follows::receive(&b, "a.example", &to_b).await.unwrap();
    let on_a = Follows::receive(&a, "b.example", &to_sub).await.unwrap();
    assert_eq!(
        (on_b.group, on_b.offer.source.clone()),
        (gb.id, group_iri("a.example", sub.id))
    );
    assert_eq!(
        (on_a.group, on_a.offer.source.clone()),
        (sub.id, group_iri("b.example", gb.id))
    );
    assert_eq!(
        before,
        (
            a.parents(sub.id).await.unwrap(),
            a.children(top.id).await.unwrap()
        )
    );
    assert!(a.parents(top.id).await.unwrap().is_empty());
    assert_eq!(a.descendants(top.id).await.unwrap(), vec![sub.id]);
    // the loop rule still works as before and is not confused by the follow
    let err = a.request_parent(top.id, sub.id).await.unwrap_err();
    assert!(matches!(err, Error::Cycle { .. }), "{err:?}");
}

#[tokio::test]
async fn following_gives_nobody_a_membership() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = b.create("B", "team").await.unwrap();
    let (_, o) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    Follows::receive(&b, "a.example", &o).await.unwrap();
    assert!(a.effective(ga.id).await.unwrap().is_empty());
    assert!(b.effective(gb.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_group_clears_its_deliveries_and_sources() {
    let ((a, sa), (b, sb)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let (_, o1) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let (_, o2) = Follows::deliver(
        &b,
        gb.id,
        &group_iri("a.example", ga.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    Follows::receive(&b, "a.example", &o1).await.unwrap();
    Follows::receive(&a, "b.example", &o2).await.unwrap();
    a.delete(ga.id).await.unwrap();
    for ty in ["Delivery", "Source"] {
        let left = sa.query(&Query::new().of_type(ty)).await.unwrap();
        assert!(left.is_empty(), "{ty} left on a: {}", left.len());
    }
    // the other site keeps its own records until it is told
    assert_eq!(
        sb.query(&Query::new().of_type("Delivery"))
            .await
            .unwrap()
            .len(),
        1
    );
    b.delete(gb.id).await.unwrap();
    assert!(
        sb.query(&Query::new().of_type("Source"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn losing_a_source_revokes_what_was_relayed_from_it() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read, Right::AddMembers]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read, Right::AddMembers]).await;
    let gc = c.create("C", "team").await.unwrap();
    let (d, o) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        true,
    )
    .await
    .unwrap();
    let s = Follows::receive(&b, "a.example", &o).await.unwrap();
    Follows::pass_on(&b, s.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap();
    let s = Follows::withdraw(&b, "a.example", &o.source, d.id)
        .await
        .unwrap();
    assert_eq!(s.state, SourceState::Withdrawn);
    let relayed = Follows::deliveries(&b, gb.id).await.unwrap();
    assert_eq!(relayed[0].state, DeliveryState::Revoked);
    // the same when the receiver removes the source itself
    let (d2, o2) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read, Right::AddMembers],
        true,
    )
    .await
    .unwrap();
    let s2 = Follows::receive(&b, "a.example", &o2).await.unwrap();
    Follows::pass_on(&b, s2.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap();
    Follows::remove_source(&b, s2.id).await.unwrap();
    let all = Follows::deliveries(&b, gb.id).await.unwrap();
    assert!(
        all.iter().all(|x| x.state == DeliveryState::Revoked),
        "{d2:?} {all:?}"
    );
}
