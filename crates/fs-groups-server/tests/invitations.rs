//! The `Invitations` interface over a real socket; the clock is the server's.

mod common;

use common::{create, gp, invitations_of, items_of, members_of, problem, with_server};
use fs_interfaces::groups_capnp::{InvitationState, invitations};
use fs_interfaces::rpc::{TOO_LARGE, read_uuid, write_uuid};
use uuid::Uuid;

const ADA: &str = "https://example.org/people/ada";
const LONG_AGO: i64 = 1_000;
const FAR_AHEAD: i64 = 4_000_000_000_000;

async fn invite(i: &invitations::Client, group: Uuid, who: &str, expires: Option<i64>) -> Uuid {
    let mut req = i.invite_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_invitee(who);
    req.get().set_role("member");
    if let Some(at) = expires {
        req.get().init_expires().set_unix_millis(at);
    }
    let answer = req.send().promise.await.unwrap();
    let inv = answer.get().unwrap().get_invitation().unwrap();
    assert_eq!(inv.get_state().unwrap(), InvitationState::Pending);
    assert_eq!(inv.get_invitee().unwrap().to_str().unwrap(), who);
    assert_eq!(read_uuid(inv.get_group().unwrap()), Some(group));
    assert_eq!(inv.get_role().unwrap().to_str().unwrap(), "member");
    assert_eq!(inv.has_expires(), expires.is_some());
    if let Some(at) = expires {
        assert_eq!(inv.get_expires().unwrap().get_unix_millis(), at);
    }
    read_uuid(inv.get_id().unwrap()).unwrap()
}

async fn states(i: &invitations::Client, group: Uuid) -> Vec<InvitationState> {
    let mut req = i.list_request();
    write_uuid(req.get().init_group(), Some(group));
    let answer = req.send().promise.await.unwrap();
    let list = answer.get().unwrap().get_items().unwrap();
    list.iter().map(|x| x.get_state().unwrap()).collect()
}

fn accept(
    i: &invitations::Client,
    id: Uuid,
    person: &str,
) -> capnp::capability::RemotePromise<invitations::accept_results::Owned> {
    let mut req = i.accept_request();
    write_uuid(req.get().init_id(), Some(id));
    req.get().set_person(person);
    req.send()
}

#[tokio::test(flavor = "current_thread")]
async fn accept_makes_the_membership_for_the_invitee_only() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let m = members_of(&root).await;
        let g = create(&items, "G").await;
        let id = invite(&i, g, ADA, Some(FAR_AHEAD)).await;

        let e = accept(&i, id, "https://example.org/people/bo")
            .promise
            .await
            .err()
            .unwrap();
        assert_eq!(problem(&e), (403, gp("not-invitee")));
        assert_eq!(states(&i, g).await, [InvitationState::Pending]);

        let answer = accept(&i, id, ADA).promise.await.unwrap();
        let membership = answer.get().unwrap().get_membership().unwrap();
        assert_eq!(membership.get_role().unwrap().to_str().unwrap(), "member");
        assert_eq!(states(&i, g).await, [InvitationState::Accepted]);

        let mut member = m.is_member_request();
        write_uuid(member.get().init_group(), Some(g));
        member.get().set_person(ADA);
        assert!(
            member
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_member()
        );

        let e = accept(&i, id, ADA).promise.await.err().unwrap();
        assert_eq!(problem(&e), (409, gp("invalid-transition")));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn accept_proved_is_for_a_mail_invitee_and_compares_the_address() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let g = create(&items, "G").await;
        let id = invite(&i, g, "ada@example.org", None).await;

        let proved = |address: &'static str| {
            let mut req = i.accept_proved_request();
            write_uuid(req.get().init_id(), Some(id));
            req.get().set_person(ADA);
            req.get().set_proved_address(address);
            req.send().promise
        };
        let e = proved("bo@example.org").await.err().unwrap();
        assert_eq!(problem(&e), (403, gp("not-invitee")));
        proved("ADA@example.org").await.unwrap();
        assert_eq!(states(&i, g).await, [InvitationState::Accepted]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn decline_revoke_and_expiry_as_read_and_as_answered() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let g = create(&items, "G").await;

        let declined = invite(&i, g, ADA, None).await;
        let mut decline = i.decline_request();
        write_uuid(decline.get().init_id(), Some(declined));
        decline.send().promise.await.unwrap();

        let revoked = invite(&i, g, ADA, None).await;
        let mut revoke = i.revoke_request();
        write_uuid(revoke.get().init_id(), Some(revoked));
        revoke.send().promise.await.unwrap();

        let swept = invite(&i, g, ADA, Some(LONG_AGO)).await;
        assert_eq!(
            states(&i, g).await,
            [
                InvitationState::Declined,
                InvitationState::Revoked,
                InvitationState::Expired
            ],
            "what ran out reads as expired without anybody answering"
        );
        let e = accept(&i, swept, ADA).promise.await.err().unwrap();
        assert_eq!(
            problem(&e),
            (410, gp("expired")),
            "the same with or without a sweep first"
        );

        let late = create(&items, "Late").await;
        let id = invite(&i, late, ADA, Some(LONG_AGO)).await;
        let e = accept(&i, id, ADA).promise.await.err().unwrap();
        assert_eq!(problem(&e), (410, gp("expired")));
        assert_eq!(states(&i, late).await, [InvitationState::Expired]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_list_holds_this_group_only_and_extreme_times_do_not_break_it() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let i = invitations_of(&root).await;
        let (g, other) = (create(&items, "G").await, create(&items, "Other").await);
        invite(&i, other, ADA, None).await;
        assert!(states(&i, g).await.is_empty());

        // The largest time `from_millis` accepts must be stored and read back, or be refused plainly.
        let largest = 8_210_266_876_799_999;
        let id = invite(&i, g, ADA, Some(largest)).await;
        assert_eq!(states(&i, g).await, [InvitationState::Pending]);
        accept(&i, id, ADA).promise.await.unwrap();

        let mut beyond = i.invite_request();
        write_uuid(beyond.get().init_group(), Some(g));
        beyond.get().set_invitee(ADA);
        beyond.get().init_expires().set_unix_millis(i64::MAX);
        let e = beyond.send().promise.await.err().unwrap();
        assert_eq!(problem(&e).0, 422);
    })
    .await;
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
        let e = accept(&i, id, mail).promise.await.err().unwrap();
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
