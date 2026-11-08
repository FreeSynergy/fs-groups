//! Following: an id of the wrong kind of object is `NotFound`, and a repeated end writes nothing.

mod common;

use std::sync::{Arc, Mutex};

use common::{group_iri, group_with, site};
use fs_groups_core::{
    DeliveryState, Error, Follows, GroupChange, GroupFeed, GroupListener, Right, SourceState,
};

#[derive(Default)]
struct Seen(Mutex<Vec<GroupChange>>);

impl GroupListener for Seen {
    fn changed(&self, change: &GroupChange) {
        self.0.lock().unwrap().push(change.clone());
    }
}

#[tokio::test]
async fn a_source_id_is_not_a_delivery_and_a_delivery_id_is_not_a_source() {
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
    let src = Follows::receive(&b, "a.example", &offer).await.unwrap();
    Follows::pass_on(&b, src.id, &group_iri("c.example", gc.id), &[Right::Read])
        .await
        .unwrap();
    let err = Follows::revoke_delivery(&b, src.id).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::NotFound {
                what: "delivery",
                ..
            }
        ),
        "{err:?}"
    );
    let relayed = Follows::deliveries(&b, gb.id).await.unwrap();
    assert_eq!(relayed[0].state, DeliveryState::Active);
    let err = Follows::remove_source(&a, deliv.id).await.unwrap_err();
    assert!(
        matches!(err, Error::NotFound { what: "source", .. }),
        "{err:?}"
    );
    assert_eq!(
        Follows::deliveries(&a, ga.id).await.unwrap()[0].state,
        DeliveryState::Active
    );
    assert_eq!(
        Follows::sources(&b, gb.id).await.unwrap()[0].state,
        SourceState::Active
    );
}

#[tokio::test]
async fn ending_twice_writes_nothing_the_second_time() {
    let ((a, _), (b, _)) = (site("a.example").await, site("b.example").await);
    let ga = group_with(&a, "a.example", "A", &[Right::Read]).await;
    let gb = b.create("B", "team").await.unwrap();
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
    let (on_a, on_b) = (Arc::new(Seen::default()), Arc::new(Seen::default()));
    let _keep = (
        GroupFeed::subscribe(&a, on_a.clone()),
        GroupFeed::subscribe(&b, on_b.clone()),
    );
    for _ in 0..2 {
        Follows::revoke_delivery(&a, d.id).await.unwrap();
        Follows::withdraw(&b, "a.example", &offer.source, d.id)
            .await
            .unwrap();
    }
    assert_eq!(
        on_a.0.lock().unwrap().len(),
        1,
        "one write for the first revoke only"
    );
    assert_eq!(
        on_b.0.lock().unwrap().len(),
        1,
        "one write for the first withdraw only"
    );
}
