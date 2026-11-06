//! Deleting a group removes what hangs on it; children stay.

mod common;

use common::{at, person, repo};
use fs_groups_core::{Delegations, Member, Right};

#[tokio::test]
async fn delete_removes_memberships_invitations_delegations_and_links() {
    let r = repo().await;
    let (parent, mid, child) = (
        r.create("p", "t").await.unwrap(),
        r.create("m", "t").await.unwrap(),
        r.create("c", "t").await.unwrap(),
    );
    r.add_parent(mid.id, parent.id).await.unwrap();
    r.add_parent(child.id, mid.id).await.unwrap();
    let ada = person("ada");
    r.add(mid.id, Member::Person(ada.clone()), "")
        .await
        .unwrap();
    r.invite(mid.id, "bo@example.org", "", Some(at(100)))
        .await
        .unwrap();
    r.grant(mid.id, Member::Person(ada.clone()), Right::Manage)
        .await
        .unwrap();
    // mid also holds a right on the parent, and ada is in the parent's own list
    r.grant(parent.id, Member::Group(mid.id), Right::Read)
        .await
        .unwrap();
    r.add(parent.id, Member::Person(ada.clone()), "")
        .await
        .unwrap();

    r.delete(mid.id).await.unwrap();

    assert!(r.get(mid.id).await.unwrap().is_none());
    assert!(r.get(child.id).await.unwrap().is_some(), "children stay");
    assert!(
        r.parents(child.id).await.unwrap().is_empty(),
        "the link to the deleted parent is gone"
    );
    assert!(r.children(parent.id).await.unwrap().is_empty());
    assert!(
        Delegations::list(&r, parent.id).await.unwrap().is_empty(),
        "rights it held are gone"
    );
    assert_eq!(
        r.direct(parent.id).await.unwrap().len(),
        1,
        "other groups keep their members"
    );
    assert_eq!(r.groups_of(&ada).await.unwrap(), [parent.id]);
    assert!(r.direct(mid.id).await.is_err());
}

#[tokio::test]
async fn deleting_an_unknown_group_is_not_found() {
    let r = repo().await;
    assert!(r.delete(fs_store::ObjectId::new()).await.is_err());
}
