//! The `Delegations` interface over a real socket. The meaning of the right flow is the core's and
//! provisional, so this only checks that the question reaches it and the setting counts.

mod common;

use common::{
    create, delegations_of, gp, items_of, members_of, nest, problem, unknown, with_server,
};
use fs_interfaces::groups_capnp::{Right, RightFlow, delegations, member};
use fs_interfaces::rpc::{read_uuid, write_uuid};
use uuid::Uuid;

const ADA: &str = "https://example.org/people/ada";

async fn may(d: &delegations::Client, group: Uuid, person: &str, right: Right) -> bool {
    let mut req = d.may_do_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_person(person);
    req.get().set_right(right);
    let answer = req.send().promise.await.unwrap();
    answer.get().unwrap().get_allowed()
}

async fn grant_person(d: &delegations::Client, group: Uuid, person: &str, right: Right) -> Uuid {
    let mut req = d.grant_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().init_holder().set_person(person);
    req.get().set_right(right);
    let answer = req.send().promise.await.unwrap();
    let delegation = answer.get().unwrap().get_delegation().unwrap();
    assert_eq!(delegation.get_right().unwrap(), right);
    read_uuid(delegation.get_id().unwrap()).unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn grant_list_may_do_revoke() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let d = delegations_of(&root).await;
        let g = create(&items, "G").await;

        let other = create(&items, "Other").await;
        grant_person(&d, other, ADA, Right::Read).await;
        assert!(
            !may(&d, g, ADA, Right::Read).await,
            "a right on another group"
        );
        let id = grant_person(&d, g, ADA, Right::AddMembers).await;
        assert_eq!(
            grant_person(&d, g, ADA, Right::AddMembers).await,
            id,
            "idempotent"
        );
        assert!(may(&d, g, ADA, Right::AddMembers).await);
        assert!(!may(&d, g, ADA, Right::RemoveMembers).await);

        let manage = grant_person(&d, g, ADA, Right::Manage).await;
        assert!(
            may(&d, g, ADA, Right::RemoveMembers).await,
            "manage implies the others"
        );

        let mut list = d.list_request();
        write_uuid(list.get().init_group(), Some(g));
        let answer = list.send().promise.await.unwrap();
        let found = answer.get().unwrap().get_items().unwrap();
        assert_eq!(found.len(), 2, "the delegation on another group stays out");
        assert_eq!(read_uuid(found.get(0).get_id().unwrap()), Some(id));
        assert_eq!(read_uuid(found.get(0).get_group().unwrap()), Some(g));
        assert_eq!(found.get(1).get_right().unwrap(), Right::Manage);
        match found.get(0).get_holder().unwrap().which().unwrap() {
            member::Which::Person(p) => assert_eq!(p.unwrap().to_str().unwrap(), ADA),
            member::Which::Group(_) => panic!("a group"),
        }

        let mut revoke = d.revoke_request();
        write_uuid(revoke.get().init_id(), Some(manage));
        revoke.send().promise.await.unwrap();
        assert!(!may(&d, g, ADA, Right::Manage).await);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_group_holds_a_right_for_its_members_and_the_setting_reaches_the_core() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let d = delegations_of(&root).await;
        let m = members_of(&root).await;
        let (top, sub, board) = (
            create(&items, "Top").await,
            create(&items, "Sub").await,
            create(&items, "Board").await,
        );
        nest(&items, sub, top).await;

        let mut add = m.add_request();
        write_uuid(add.get().init_group(), Some(board));
        add.get().init_member().set_person(ADA);
        add.send().promise.await.unwrap();

        let mut grant = d.grant_request();
        write_uuid(grant.get().init_group(), Some(top));
        write_uuid(grant.get().init_holder().init_group(), Some(board));
        grant.get().set_right(Right::Read);
        grant.send().promise.await.unwrap();

        assert!(
            may(&d, top, ADA, Right::Read).await,
            "through the holder group"
        );
        assert!(
            !may(&d, sub, ADA, Right::Read).await,
            "none passes nothing on"
        );

        for (group, flow) in [(top, RightFlow::Down), (sub, RightFlow::Down)] {
            let mut set = items.set_right_flow_request();
            write_uuid(set.get().init_id(), Some(group));
            set.get().set_flow(flow);
            set.send().promise.await.unwrap();
        }
        assert!(may(&d, sub, ADA, Right::Read).await, "the setting counts");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn refusals() {
    with_server(|root| async move {
        let d = delegations_of(&root).await;
        let mut revoke = d.revoke_request();
        write_uuid(revoke.get().init_id(), Some(unknown()));
        let e = revoke.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (404, gp("not-found")));

        let mut asked = d.may_do_request();
        write_uuid(asked.get().init_group(), Some(unknown()));
        asked.get().set_person(ADA);
        let e = asked.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (404, gp("not-found")));
    })
    .await;
}
