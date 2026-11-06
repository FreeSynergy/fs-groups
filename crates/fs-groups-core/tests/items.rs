//! Group items: create, read, change, list.

mod common;

use common::repo;
use fs_groups_core::Error;
use fs_store::ObjectId;

#[tokio::test]
async fn a_group_is_an_as2_group_in_an_envelope() {
    let r = repo().await;
    let g = r.create("Garden club", "club").await.unwrap();
    assert_eq!(g.iri, format!("https://example.org/groups/{}", g.id));
    let as2 = g.as2();
    assert_eq!(as2["type"], "Group");
    assert_eq!(as2["name"], "Garden club");
    assert_eq!(as2["fs:kind"], "club");
    assert_eq!(r.get(g.id).await.unwrap(), Some(g));
}

#[tokio::test]
async fn kind_is_free_text_and_name_and_kind_can_change() {
    let r = repo().await;
    let g = r
        .create("Solar", "working group of the third kind")
        .await
        .unwrap();
    assert_eq!(g.kind, "working group of the third kind");
    let g = r.rename(g.id, "Solar roofs").await.unwrap();
    let g = r.set_kind(g.id, "project").await.unwrap();
    let got = r.get(g.id).await.unwrap().unwrap();
    assert_eq!(
        (got.name.as_str(), got.kind.as_str()),
        ("Solar roofs", "project")
    );
}

#[tokio::test]
async fn a_blank_name_is_refused_and_unknown_ids_are_not_found() {
    let r = repo().await;
    assert!(matches!(
        r.create("  ", "team").await,
        Err(Error::Conflict(_))
    ));
    assert!(r.get(ObjectId::new()).await.unwrap().is_none());
    assert!(matches!(
        r.rename(ObjectId::new(), "x").await,
        Err(Error::NotFound { .. })
    ));
}

#[tokio::test]
async fn listing_is_in_name_order_and_paged() {
    let r = repo().await.with_page_size(2);
    for n in ["delta", "Alpha", "charlie", "Bravo", "echo"] {
        r.create(n, "team").await.unwrap();
    }
    let mut names = vec![];
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = r.list(cursor.as_ref()).await.unwrap();
        names.extend(page.items.into_iter().map(|g| g.name));
        pages += 1;
        match page.next {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    assert_eq!(names, ["Alpha", "Bravo", "charlie", "delta", "echo"]);
    assert_eq!(pages, 3);
}

#[tokio::test]
async fn the_store_holds_an_envelope_with_derived_name_and_context() {
    let (r, store) = common::setup().await;
    let g = r.create("Council", "body").await.unwrap();
    let o = store.get(g.id).await.unwrap().unwrap();
    assert_eq!(o.envelope.object_type, "Group");
    assert_eq!(o.envelope.context.as_deref(), Some("groups"));
    assert_eq!(o.envelope.name, "Council");
    assert_eq!(o.envelope.iri.as_deref(), Some(g.iri.as_str()));
}
