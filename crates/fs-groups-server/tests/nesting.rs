//! The nesting methods of `Items`: a link only after approval, requests and their order, cycles.

mod common;

use common::{create, gp, ids, items_of, nest, problem, request, with_server};
use fs_interfaces::groups_capnp::RequestState;
use fs_interfaces::rpc::{read_uuid, write_uuid};

macro_rules! read_ids {
    ($items:expr, $request:ident, $id:expr) => {{
        let mut req = $items.$request();
        write_uuid(req.get().init_id(), Some($id));
        let answer = req.send().promise.await.unwrap();
        ids(answer.get().unwrap().get_ids().unwrap())
    }};
}

#[tokio::test(flavor = "current_thread")]
async fn a_link_exists_only_after_approval_and_reads_follow_it() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let (top, mid, leaf) = (
            create(&items, "Top").await,
            create(&items, "Mid").await,
            create(&items, "Leaf").await,
        );
        let elsewhere = create(&items, "Elsewhere").await;
        request(&items, leaf, elsewhere).await;
        let asked = request(&items, mid, top).await;
        assert!(
            read_ids!(items, parents_request, mid).is_empty(),
            "linked before approval"
        );

        let mut list = items.parent_requests_request();
        write_uuid(list.get().init_parent(), Some(top));
        let answer = list.send().promise.await.unwrap();
        let requests = answer.get().unwrap().get_items().unwrap();
        assert_eq!(requests.len(), 1, "the request to another parent stays out");
        let first = requests.get(0);
        assert_eq!(read_uuid(first.get_id().unwrap()), Some(asked));
        assert_eq!(read_uuid(first.get_child().unwrap()), Some(mid));
        assert_eq!(read_uuid(first.get_parent().unwrap()), Some(top));
        assert_eq!(first.get_state().unwrap(), RequestState::Pending);
        assert_eq!(first.get_requester().unwrap().to_str().unwrap(), common::ME);

        let mut approve = items.approve_parent_request();
        write_uuid(approve.get().init_request(), Some(asked));
        let approved = approve.send().promise.await.unwrap();
        let child = approved.get().unwrap().get_group().unwrap();
        assert_eq!(ids(child.get_parents().unwrap()), [top]);

        let mut read = items.parent_requests_request();
        write_uuid(read.get().init_parent(), Some(top));
        let answer = read.send().promise.await.unwrap();
        let after = answer.get().unwrap().get_items().unwrap();
        assert_eq!(after.get(0).get_state().unwrap(), RequestState::Approved);

        nest(&items, leaf, mid).await;
        assert_eq!(read_ids!(items, parents_request, leaf), [mid]);
        assert_eq!(read_ids!(items, children_request, top), [mid]);
        assert_eq!(read_ids!(items, ancestors_request, leaf), [mid, top]);
        assert_eq!(read_ids!(items, descendants_request, top), [mid, leaf]);

        let mut remove = items.remove_parent_request();
        write_uuid(remove.get().init_id(), Some(leaf));
        write_uuid(remove.get().init_parent(), Some(mid));
        let removed = remove.send().promise.await.unwrap();
        let g = removed.get().unwrap().get_group().unwrap();
        assert_eq!(ids(g.get_parents().unwrap()), [] as [uuid::Uuid; 0]);
        assert_eq!(
            read_ids!(items, children_request, mid),
            [] as [uuid::Uuid; 0]
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn parent_requests_come_oldest_first_with_their_state() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let top = create(&items, "Top").await;
        let mut asked = vec![];
        for n in 0..5 {
            let child = create(&items, &format!("{}", 9 - n)).await;
            asked.push(request(&items, child, top).await);
        }
        let mut decline = items.decline_parent_request();
        write_uuid(decline.get().init_request(), Some(asked[1]));
        decline.send().promise.await.unwrap();

        let mut list = items.parent_requests_request();
        write_uuid(list.get().init_parent(), Some(top));
        let answer = list.send().promise.await.unwrap();
        let found = answer.get().unwrap().get_items().unwrap();
        let got: Vec<_> = found
            .iter()
            .map(|r| read_uuid(r.get_id().unwrap()).unwrap())
            .collect();
        assert_eq!(got, asked);
        assert_eq!(found.get(1).get_state().unwrap(), RequestState::Declined);
        assert_eq!(found.get(0).get_state().unwrap(), RequestState::Pending);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn declining_leaves_no_link_and_a_second_answer_is_refused() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let (a, b) = (create(&items, "A").await, create(&items, "B").await);
        let asked = request(&items, a, b).await;
        let mut decline = items.decline_parent_request();
        write_uuid(decline.get().init_request(), Some(asked));
        decline.send().promise.await.unwrap();
        assert_eq!(read_ids!(items, parents_request, a), [] as [uuid::Uuid; 0]);

        let mut again = items.approve_parent_request();
        write_uuid(again.get().init_request(), Some(asked));
        let e = again.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (409, gp("conflict")));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_cycle_is_refused_when_asking_and_when_approving() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let (a, b) = (create(&items, "A").await, create(&items, "B").await);

        let mut own = items.request_parent_request();
        write_uuid(own.get().init_child(), Some(a));
        write_uuid(own.get().init_parent(), Some(a));
        assert_eq!(
            problem(&own.send().promise.await.err().unwrap()),
            (409, gp("cycle"))
        );

        let (up, down) = (request(&items, a, b).await, request(&items, b, a).await);
        let mut first = items.approve_parent_request();
        write_uuid(first.get().init_request(), Some(up));
        first.send().promise.await.unwrap();
        let mut second = items.approve_parent_request();
        write_uuid(second.get().init_request(), Some(down));
        let e = second.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (409, gp("cycle")));
        assert_eq!(read_ids!(items, parents_request, b), [] as [uuid::Uuid; 0]);
    })
    .await;
}
