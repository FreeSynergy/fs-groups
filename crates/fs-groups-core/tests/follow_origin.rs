//! Following: a relayed copy keeps the first source, and only passes on what the actor holds itself.

mod common;

use common::{group_iri, group_with, site};
use fs_groups_core::{Error, Follows, Right};

#[tokio::test]
async fn the_actor_must_hold_the_rights_it_passes_on() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = b.create("B", "team").await.unwrap(); // the actor holds nothing on it
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
    let err = Follows::pass_on(&b, s.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err:?}");
    assert_eq!(
        Follows::deliveries(&b, gb.id).await.unwrap(),
        [] as [fs_groups_core::Delivery; 0]
    );
}

#[tokio::test]
async fn the_origin_stays_the_first_source_along_the_chain() {
    let ((a, _), (b, _), (c, _)) = (
        site("a.example").await,
        site("b.example").await,
        site("c.example").await,
    );
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = group_with(&b, "b.example", "B", &[Right::Read]).await;
    let gc = c.create("C", "team").await.unwrap();
    let (deliv, offer) = Follows::deliver(
        &a,
        ga.id,
        &group_iri("b.example", gb.id),
        &[Right::Read],
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        (deliv.origin.as_str(), offer.origin.as_str()),
        (offer.source.as_str(), offer.source.as_str())
    );
    let src = Follows::receive(&b, "a.example", &offer).await.unwrap();
    let (d2, relay) = Follows::pass_on(&b, src.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap();
    let a_iri = group_iri("a.example", ga.id);
    assert_eq!(relay.source, group_iri("b.example", gb.id), "who sent it");
    assert_eq!(
        (relay.origin.as_str(), d2.origin.as_str()),
        (a_iri.as_str(), a_iri.as_str())
    );
    let got = Follows::receive(&c, "b.example", &relay).await.unwrap();
    assert_eq!(got.offer.origin, a_iri);
    // it does not go back to where it came from
    let back = Follows::pass_on(&b, src.id, &a_iri, &[Right::Read])
        .await
        .unwrap_err();
    assert!(matches!(back, Error::Invalid(_)), "{back:?}");
}
