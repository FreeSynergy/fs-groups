//! `Follows` over a real socket: deliver, list, revoke on the giver side; receive, list, remove on the
//! receiver side. What a follow means is the core's (provisional); this checks the wire.

mod common;

use common::{ME, create, delegations_of, gp, items_of, problem, unknown, with_server};
use fs_interfaces::groups_capnp::{DeliveryState, Right, SourceState, follows, groups};
use fs_interfaces::rpc::{read_uuid, write_uuid};
use uuid::Uuid;

const OTHER: &str = "other.example";

async fn follows_of(root: &groups::Client) -> follows::Client {
    let answer = root.follows_request().send().promise.await.unwrap();
    answer.get().unwrap().get_follows().unwrap()
}

fn group_iri(host: &str, id: Uuid) -> String {
    format!("https://{host}/groups/{id}")
}

async fn deliver(
    f: &follows::Client,
    group: Uuid,
    to: &str,
    rights: &[Right],
) -> Result<(Uuid, String), capnp::Error> {
    let mut req = f.deliver_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_recipient(to);
    let mut list = req.get().init_rights(u32::try_from(rights.len()).unwrap());
    for (i, r) in (0u32..).zip(rights) {
        list.set(i, *r);
    }
    let answer = req.send().promise.await?;
    let r = answer.get().unwrap();
    let delivery = r.get_delivery().unwrap();
    let offer = r.get_offer().unwrap();
    assert_eq!(
        read_uuid(offer.get_delivery().unwrap()).unwrap(),
        read_uuid(delivery.get_id().unwrap()).unwrap()
    );
    assert_eq!(delivery.get_state().unwrap(), DeliveryState::Active);
    Ok((
        read_uuid(delivery.get_id().unwrap()).unwrap(),
        offer.get_source().unwrap().to_string().unwrap(),
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn the_giver_delivers_lists_and_revokes() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let d = delegations_of(&root).await;
        let f = follows_of(&root).await;
        let g = create(&items, "G").await;
        let mut grant = d.grant_request();
        write_uuid(grant.get().init_group(), Some(g));
        grant.get().init_holder().set_person(ME);
        grant.get().set_right(Right::Read);
        grant.send().promise.await.unwrap();

        let to = group_iri("other.example", unknown());
        let (delivery, source) = deliver(&f, g, &to, &[Right::Read]).await.unwrap();
        assert_eq!(source, group_iri("local.invalid", g));

        // never more than the giver holds
        let refused = deliver(&f, g, &to, &[Right::AddMembers])
            .await
            .err()
            .unwrap();
        assert_eq!(problem(&refused).0, 403, "{refused:?}");

        let mut list = f.deliveries_request();
        write_uuid(list.get().init_group(), Some(g));
        let answer = list.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_items().unwrap().len(), 1);

        let mut revoke = f.revoke_request();
        write_uuid(revoke.get().init_delivery(), Some(delivery));
        revoke.send().promise.await.unwrap();
        let mut list = f.deliveries_request();
        write_uuid(list.get().init_group(), Some(g));
        let answer = list.send().promise.await.unwrap();
        let items = answer.get().unwrap().get_items().unwrap();
        assert_eq!(items.get(0).get_state().unwrap(), DeliveryState::Revoked);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_receiver_enters_lists_and_removes_a_source() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let f = follows_of(&root).await;
        let mine = create(&items, "Mine").await;
        let delivery = unknown();
        let mut receive = f.receive_request();
        receive.get().set_sender(OTHER);
        {
            let mut offer = receive.get().init_offer();
            write_uuid(offer.reborrow().init_delivery(), Some(delivery));
            offer
                .reborrow()
                .set_source(group_iri("other.example", unknown()));
            offer
                .reborrow()
                .set_recipient(group_iri("local.invalid", mine));
            offer.reborrow().init_rights(1).set(0, Right::Read);
            offer.set_origin(group_iri("other.example", unknown()));
        }
        let answer = receive.send().promise.await.unwrap();
        let source = answer.get().unwrap().get_source().unwrap();
        assert_eq!(source.get_state().unwrap(), SourceState::Active);
        let id = read_uuid(source.get_id().unwrap()).unwrap();

        // a sender that is not the origin of the source is refused
        let mut forged = f.receive_request();
        forged.get().set_sender("evil.example");
        {
            let mut offer = forged.get().init_offer();
            write_uuid(offer.reborrow().init_delivery(), Some(unknown()));
            offer
                .reborrow()
                .set_source(group_iri("other.example", unknown()));
            offer
                .reborrow()
                .set_recipient(group_iri("local.invalid", mine));
            offer.reborrow().init_rights(1).set(0, Right::Read);
            offer.set_origin(group_iri("other.example", unknown()));
        }
        let err = forged.send().promise.await.err().unwrap();
        assert_eq!(problem(&err), (403, gp("forbidden")));

        let mut sources = f.sources_request();
        write_uuid(sources.get().init_group(), Some(mine));
        let answer = sources.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_items().unwrap().len(), 1);

        let mut remove = f.remove_source_request();
        write_uuid(remove.get().init_id(), Some(id));
        remove.send().promise.await.unwrap();
        let mut sources = f.sources_request();
        write_uuid(sources.get().init_group(), Some(mine));
        let answer = sources.send().promise.await.unwrap();
        let list = answer.get().unwrap().get_items().unwrap();
        assert!(list.is_empty() || list.get(0).get_state().unwrap() == SourceState::Withdrawn);
    })
    .await;
}
