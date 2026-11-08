//! Following: never more than the giver has, passing on only with permission, the sender is the origin.
//! Tests against the `Follows` trait (provisional, ADR 0056).

mod common;

use common::{group_iri, group_with, site};
use fs_groups_core::{Error, Follows, Right};
use fs_objects::ObjectId;

#[tokio::test]
async fn more_rights_than_the_giver_has_are_refused() {
    let (a, _) = site("a.example").await;
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read]).await;
    let to = group_iri("b.example", ObjectId::new());
    let both = [Right::Read, Right::AddMembers];
    let err = Follows::deliver(&a, ga.id, &to, &both, false)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
    assert!(Follows::deliveries(&a, ga.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn manage_is_never_delivered() {
    let (a, _) = site("a.example").await;
    let ga = group_with(&a, "a.example", "Giver", &[Right::Read, Right::AddMembers]).await;
    let to = group_iri("b.example", ObjectId::new());
    let err = Follows::deliver(&a, ga.id, &to, &[Right::Manage], false)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
}

#[tokio::test]
async fn passing_on_without_permission_is_refused() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let gc = c.create("C", "team").await.unwrap();
    let (_, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let s = Follows::receive(&b, "a.example", &offer).await.unwrap();
    let err = Follows::pass_on(&b, s.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
    assert!(Follows::sources(&c, gc.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn passing_on_with_permission_works_within_what_was_given() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read, Right::AddMembers]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let gc = c.create("C", "team").await.unwrap();
    let to_c = group_iri("c.example", gc.id);
    let (_, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        true,
    )
    .await
    .unwrap();
    let s = Follows::receive(&b, "a.example", &offer).await.unwrap();
    let err = Follows::pass_on(&b, s.id, &to_c, &[Right::AddMembers])
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Forbidden(_)),
        "more than was given: {err:?}"
    );
    let (d, relay) = Follows::pass_on(&b, s.id, &to_c, &[Right::Read])
        .await
        .unwrap();
    assert_eq!(d.via, Some(s.id));
    let got = Follows::receive(&c, "b.example", &relay).await.unwrap();
    assert_eq!(got.offer.rights, vec![Right::Read]);
}

#[tokio::test]
async fn a_relayed_copy_is_not_passable_again() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let gc = c.create("C", "team").await.unwrap();
    let (_, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        true,
    )
    .await
    .unwrap();
    let s = Follows::receive(&b, "a.example", &offer).await.unwrap();
    let (_, relay) = Follows::pass_on(&b, s.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap();
    assert!(!relay.pass_on);
    let sc = Follows::receive(&c, "b.example", &relay).await.unwrap();
    let err = Follows::pass_on(&c, sc.id, &group_iri("a.example", ga.id), &[Right::Read])
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
}

#[tokio::test]
async fn an_offer_from_another_origin_is_refused() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = b.create("B", "team").await.unwrap();
    let (_, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        false,
    )
    .await
    .unwrap();
    let err = Follows::receive(&b, "evil.example", &offer)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
    assert!(Follows::sources(&b, gb.id).await.unwrap().is_empty());
    // a withdrawal only counts from the one that sent the offer
    Follows::receive(&b, "a.example", &offer).await.unwrap();
    let err = Follows::withdraw(&b, "evil.example", &offer.source, offer.delivery)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
}

#[tokio::test]
async fn an_unknown_group_is_an_error() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let ghost = ObjectId::new();
    let nf = |e: Error| matches!(e, Error::NotFound { what: "group", .. });
    let to = group_iri("b.example", ObjectId::new());
    assert!(nf(Follows::deliver(&a, ghost, &to, &[Right::Read], false)
        .await
        .unwrap_err()));
    assert!(nf(Follows::deliveries(&a, ghost).await.unwrap_err()));
    assert!(nf(Follows::sources(&b, ghost).await.unwrap_err()));
    let (_, offer) = Follows::deliver(&a, ga.id, &to, &[Right::Read], false)
        .await
        .unwrap();
    assert!(nf(Follows::receive(&b, "a.example", &offer)
        .await
        .unwrap_err()));
}
