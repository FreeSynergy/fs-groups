//! The rules hold when calls overlap on the socket: the server opens no way around the core's lock.

mod common;

use common::{create, gp, ids, invitations_of, items_of, problem, request, with_server};
use fs_interfaces::rpc::{read_uuid, write_uuid};

#[tokio::test(flavor = "current_thread")]
async fn two_approvals_cannot_close_a_loop_together() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let (a, b) = (create(&items, "A").await, create(&items, "B").await);
        let (up, down) = (request(&items, a, b).await, request(&items, b, a).await);

        let mut one = items.approve_parent_request();
        write_uuid(one.get().init_request(), Some(up));
        let mut two = items.approve_parent_request();
        write_uuid(two.get().init_request(), Some(down));
        let (one, two) = tokio::join!(one.send().promise, two.send().promise);
        let refused = [one.err(), two.err()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(refused.len(), 1, "exactly one wins");
        assert_eq!(problem(&refused[0]), (409, gp("cycle")));

        let mut parents = items.ancestors_request();
        write_uuid(parents.get().init_id(), Some(a));
        let answer = parents.send().promise.await.unwrap();
        let of_a = ids(answer.get().unwrap().get_ids().unwrap());
        assert!(of_a.is_empty() || of_a == [b]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn one_invitation_has_one_answer() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let g = create(&items, "G").await;
        let person = "https://example.org/people/ada";
        let mut invite = i.invite_request();
        write_uuid(invite.get().init_group(), Some(g));
        invite.get().set_invitee(person);
        let answer = invite.send().promise.await.unwrap();
        let id = read_uuid(
            answer
                .get()
                .unwrap()
                .get_invitation()
                .unwrap()
                .get_id()
                .unwrap(),
        )
        .unwrap();

        let mut accept = i.accept_request();
        write_uuid(accept.get().init_id(), Some(id));
        accept.get().set_person(person);
        let mut decline = i.decline_request();
        write_uuid(decline.get().init_id(), Some(id));
        let (accepted, declined) = tokio::join!(accept.send().promise, decline.send().promise);
        assert!(
            accepted.is_ok() != declined.is_ok(),
            "exactly one answer counts"
        );
    })
    .await;
}
