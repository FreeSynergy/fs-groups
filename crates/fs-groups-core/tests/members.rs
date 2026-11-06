//! Memberships: org:Membership relationships with role ids.

mod common;

use common::{person, repo, setup};
use fs_groups_core::{Error, Member};
use fs_store::ObjectId;

#[tokio::test]
async fn a_person_joins_with_a_role_and_is_listed() {
    let r = repo().await;
    let g = r.create("Board", "body").await.unwrap();
    let m = r
        .add(g.id, Member::Person(person("ada")), "member")
        .await
        .unwrap();
    r.add(g.id, Member::Person(person("bo")), "").await.unwrap();
    let direct = r.direct(g.id).await.unwrap();
    assert_eq!(direct.len(), 2);
    assert_eq!(direct[0], m);
    assert_eq!(direct[1].role, "", "empty = plain member");
}

#[tokio::test]
async fn a_membership_is_an_as2_relationship_in_org_membership_form() {
    let (r, store) = setup().await;
    let g = r.create("Board", "body").await.unwrap();
    let m = r
        .add(g.id, Member::Person(person("ada")), "user")
        .await
        .unwrap();
    let o = store.get(m.id).await.unwrap().unwrap();
    assert_eq!(o.envelope.object_type, "Relationship");
    assert_eq!(o.envelope.context.as_deref(), Some("groups"));
    assert_eq!(o.content["relationship"], "org:Membership");
    assert_eq!(o.content["subject"], person("ada"));
    assert_eq!(o.content["object"], g.iri);
    assert_eq!(o.content["org:role"], "user");
}

#[tokio::test]
async fn the_same_person_and_role_twice_is_one_membership_another_role_is_another() {
    let r = repo().await;
    let g = r.create("Board", "body").await.unwrap();
    r.add(g.id, Member::Person(person("ada")), "member")
        .await
        .unwrap();
    // the same triple again is the same membership, not a second one
    let first = r.direct(g.id).await.unwrap().remove(0);
    let again = r
        .add(
            g.id,
            Member::Person(format!(" {} ", person("ada"))),
            "member",
        )
        .await
        .unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(
        again.member,
        Member::Person(person("ada")),
        "stored trimmed"
    );
    r.add(g.id, Member::Person(person("ada")), "user")
        .await
        .unwrap();
    assert_eq!(r.direct(g.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn a_group_as_member_is_refused_nesting_has_its_own_call() {
    let r = repo().await;
    let (a, b) = (
        r.create("a", "team").await.unwrap(),
        r.create("b", "team").await.unwrap(),
    );
    let err = r.add(a.id, Member::Group(b.id), "").await.unwrap_err();
    assert!(err.to_string().contains("request_parent"));
}

#[tokio::test]
async fn removing_ends_the_membership_and_unknown_ids_are_not_found() {
    let r = repo().await;
    let g = r.create("Board", "body").await.unwrap();
    let m = r
        .add(g.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.remove(m.id).await.unwrap();
    assert!(r.direct(g.id).await.unwrap().is_empty());
    assert!(matches!(r.remove(m.id).await, Err(Error::NotFound { .. })));
    assert!(matches!(
        r.add(ObjectId::new(), Member::Person(person("x")), "")
            .await,
        Err(Error::NotFound { .. })
    ));
    assert!(
        matches!(r.remove(g.id).await, Err(Error::NotFound { .. })),
        "a group is not a membership"
    );
}

#[tokio::test]
async fn is_member_and_groups_of_trim_the_person_as_add_does() {
    let r = repo().await;
    let g = r.create("Club", "").await.unwrap();
    r.add(g.id, Member::Person(format!(" {} ", person("ada"))), "")
        .await
        .unwrap();
    let spaced = format!("  {}\n", person("ada"));
    assert!(r.is_member(g.id, &spaced).await.unwrap());
    assert_eq!(r.groups_of(&spaced).await.unwrap(), [g.id]);
    assert!(r.groups_of("   ").await.unwrap().is_empty());
}
