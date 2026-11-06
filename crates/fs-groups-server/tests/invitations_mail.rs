//! Invitations to a mail address and expiry through `acceptProved`; small identifiers.

mod common;

use common::{create, gp, invitations_of, items_of, problem, with_server};
use fs_interfaces::groups_capnp::{InvitationState, invitations};
use fs_interfaces::rpc::{TOO_LARGE, read_uuid, write_uuid};
use uuid::Uuid;

const ADA: &str = "https://example.org/people/ada";
const LONG_AGO: i64 = 1_000;

async fn invite(i: &invitations::Client, group: Uuid, who: &str, expires: Option<i64>) -> Uuid {
    let mut req = i.invite_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_invitee(who);
    if let Some(at) = expires {
        req.get().init_expires().set_unix_millis(at);
    }
    let answer = req.send().promise.await.unwrap();
    read_uuid(
        answer
            .get()
            .unwrap()
            .get_invitation()
            .unwrap()
            .get_id()
            .unwrap(),
    )
    .unwrap()
}

async fn states(i: &invitations::Client, group: Uuid) -> Vec<InvitationState> {
    let mut req = i.list_request();
    write_uuid(req.get().init_group(), Some(group));
    let answer = req.send().promise.await.unwrap();
    let list = answer.get().unwrap().get_items().unwrap();
    list.iter().map(|x| x.get_state().unwrap()).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn accept_proved_of_an_expired_invitation_answers_expired_before_and_after_a_sweep() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let g = create(&items, "G").await;
        let mail = "ada@example.org";
        let id = invite(&i, g, mail, Some(LONG_AGO)).await;

        let proved = || {
            let mut req = i.accept_proved_request();
            write_uuid(req.get().init_id(), Some(id));
            req.get().set_person(ADA);
            req.get().set_proved_address(mail);
            req.send().promise
        };
        assert_eq!(
            problem(&proved().await.err().unwrap()),
            (410, gp("expired"))
        );
        assert_eq!(states(&i, g).await, [InvitationState::Expired]);
        assert_eq!(
            problem(&proved().await.err().unwrap()),
            (410, gp("expired"))
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_mail_invitation_needs_the_proof_and_identifiers_are_small() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let g = create(&items, "G").await;
        let mail = "ada@example.org";
        let id = invite(&i, g, mail, None).await;
        let mut said = i.accept_request();
        write_uuid(said.get().init_id(), Some(id));
        said.get().set_person(mail);
        let e = said.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (403, gp("not-invitee")));
        assert_eq!(states(&i, g).await, [InvitationState::Pending]);

        let mut long = i.invite_request();
        write_uuid(long.get().init_group(), Some(g));
        long.get().set_invitee("x".repeat(4097).as_str());
        let e = long.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (413, TOO_LARGE.to_string()));
    })
    .await;
}
