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
    common::link(&r, mid.id, parent.id).await;
    common::link(&r, child.id, mid.id).await;
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
    assert_eq!(
        r.children(parent.id).await.unwrap(),
        [] as [fs_objects::ObjectId; 0]
    );
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
    assert!(r.delete(fs_objects::ObjectId::new()).await.is_err());
}

#[tokio::test]
async fn a_group_holding_a_right_on_itself_can_be_deleted() {
    let r = repo().await;
    let g = r.create("Self", "").await.unwrap();
    r.add(g.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.grant(g.id, Member::Group(g.id), Right::Manage)
        .await
        .unwrap();
    r.delete(g.id).await.unwrap();
    assert!(r.get(g.id).await.unwrap().is_none());
    assert_eq!(
        r.groups_of(&person("ada")).await.unwrap(),
        [] as [fs_objects::ObjectId; 0]
    );
}
