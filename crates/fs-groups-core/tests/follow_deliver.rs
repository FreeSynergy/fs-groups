//! Following: the giver delivers, the receiver is told and enters the source, nobody confirms.
//! Tests against the `Follows` trait (provisional, ADR 0056).

mod common;

use common::{group_iri, group_with, site};
use fs_groups_core::{Follows, Right, SourceState};

#[tokio::test]
async fn a_delivery_is_entered_without_confirmation() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read]).await;
    let gb = b.create("Taker", "team").await.unwrap();
    let giver: &dyn Follows = &a;
    let taker: &dyn Follows = &b;
    let (d, offer) = giver
        .deliver(ga.id, &group_iri("b.example", gb.id), &[Right::Read], false)
        .await
        .unwrap();
    let s = taker.receive("a.example", &offer).await.unwrap();
    assert_eq!((s.group, s.state), (gb.id, SourceState::Active));
    assert_eq!(s.offer.delivery, d.id);
    assert_eq!(taker.sources(gb.id).await.unwrap(), vec![s]);
}

#[tokio::test]
async fn the_receiver_sees_what_is_offered() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read, Right::AddMembers]).await;
    let gb = b.create("Taker", "team").await.unwrap();
    let rights = [Right::Read, Right::AddMembers];
    let (_, offer) = Follows::deliver(&a, ga.id, &group_iri("b.example", gb.id), &rights, true)
        .await
        .unwrap();
    Follows::receive(&b, "a.example", &offer).await.unwrap();
    let seen = &Follows::sources(&b, gb.id).await.unwrap()[0];
    assert_eq!(seen.offer.source, group_iri("a.example", ga.id));
    assert_eq!(seen.offer.rights, rights);
    assert!(seen.offer.pass_on);
    assert_eq!(seen.sender, "a.example");
}

#[tokio::test]
async fn receiving_the_same_offer_twice_keeps_one_source() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read]).await;
    let gb = b.create("Taker", "team").await.unwrap();
    let (_, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let first = Follows::receive(&b, "a.example", &offer).await.unwrap();
    let again = Follows::receive(&b, "a.example", &offer).await.unwrap();
    assert_eq!(first, again);
    assert_eq!(Follows::sources(&b, gb.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_receiver_can_remove_the_source() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read]).await;
    let gb = b.create("Taker", "team").await.unwrap();
    let (d, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let s = Follows::receive(&b, "a.example", &offer).await.unwrap();
    Follows::remove_source(&b, s.id).await.unwrap();
    assert!(Follows::sources(&b, gb.id).await.unwrap().is_empty());
    // the giver is not asked and keeps its delivery
    let kept = Follows::deliveries(&a, ga.id).await.unwrap();
    assert_eq!(kept[0].id, d.id);
}

#[tokio::test]
async fn the_giver_revokes_at_any_time() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read]).await;
    let gb = b.create("Taker", "team").await.unwrap();
    let (d, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    Follows::receive(&b, "a.example", &offer).await.unwrap();
    Follows::revoke_delivery(&a, d.id).await.unwrap();
    let after = Follows::deliveries(&a, ga.id).await.unwrap();
    assert_eq!(after[0].state, fs_groups_core::DeliveryState::Revoked);
    let s = Follows::withdraw(&b, "a.example", &offer.source, d.id)
        .await
        .unwrap();
    assert_eq!(s.state, SourceState::Withdrawn);
    Follows::revoke_delivery(&a, d.id).await.unwrap(); // again: nothing changes
}

#[tokio::test]
async fn following_may_be_mutual() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let (_, to_b) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let (_, to_a) = Follows::deliver(
        &b,
        gb.id,
        &group_iri("a.example", ga.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    Follows::receive(&b, "a.example", &to_b).await.unwrap();
    Follows::receive(&a, "b.example", &to_a).await.unwrap();
    assert_eq!(Follows::sources(&a, ga.id).await.unwrap().len(), 1);
    assert_eq!(Follows::sources(&b, gb.id).await.unwrap().len(), 1);
}
