//! Invitations: the life cycle and its single transition function.

mod common;

use common::{at, person, repo};
use fs_groups_core::{Error, InvitationState, Invitations, Step};

#[tokio::test]
async fn accepting_creates_the_membership_with_the_offered_role() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r
        .invite(g.id, "ada@example.org", "member", Some(at(1000)))
        .await
        .unwrap();
    assert_eq!(inv.state, InvitationState::Pending);
    // sent to an e-mail address: the caller says which verified address the person proved
    let m = r
        .accept_proved(inv.id, &person("ada"), "Ada@Example.org", at(500))
        .await
        .unwrap();
    assert_eq!((m.group, m.role.as_str()), (g.id, "member"));
    assert!(r.is_member(g.id, &person("ada")).await.unwrap());
    assert_eq!(
        Invitations::list(&r, g.id).await.unwrap()[0].state,
        InvitationState::Accepted
    );
}

#[tokio::test]
async fn an_answered_invitation_cannot_be_answered_again() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r.invite(g.id, &person("ada"), "", None).await.unwrap();
    Invitations::decline(&r, inv.id).await.unwrap();
    let err = Invitations::accept(&r, inv.id, &person("ada"), at(1))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidTransition {
            from: InvitationState::Declined,
            ..
        }
    ));
    assert_eq!(
        r.direct(g.id).await.unwrap(),
        [] as [fs_groups_core::Membership; 0]
    );
}

#[tokio::test]
async fn a_revoked_invitation_cannot_be_accepted() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r.invite(g.id, &person("ada"), "", None).await.unwrap();
    Invitations::revoke(&r, inv.id).await.unwrap();
    assert!(
        Invitations::accept(&r, inv.id, &person("ada"), at(1))
            .await
            .is_err()
    );
    assert_eq!(
        Invitations::list(&r, g.id).await.unwrap()[0].state,
        InvitationState::Revoked
    );
}

#[tokio::test]
async fn an_expired_invitation_cannot_be_accepted_and_is_stored_as_expired() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r
        .invite(g.id, &person("ada"), "", Some(at(1000)))
        .await
        .unwrap();
    // the moment itself counts as passed
    let err = Invitations::accept(&r, inv.id, &person("ada"), at(1000))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Expired(id) if id == inv.id));
    assert_eq!(
        r.direct(g.id).await.unwrap(),
        [] as [fs_groups_core::Membership; 0]
    );
    assert_eq!(
        Invitations::list(&r, g.id).await.unwrap()[0].state,
        InvitationState::Expired
    );
    assert!(
        Invitations::accept(&r, inv.id, &person("ada"), at(1))
            .await
            .is_err(),
        "final, even for an earlier clock"
    );
}

#[tokio::test]
async fn expire_due_marks_only_the_pending_ones_that_ran_out() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let old = r.invite(g.id, "a", "", Some(at(10))).await.unwrap();
    let fresh = r.invite(g.id, "b", "", Some(at(100))).await.unwrap();
    let never = r.invite(g.id, "c", "", None).await.unwrap();
    assert_eq!(r.expire_due(at(50)).await.unwrap(), 1);
    assert_eq!(r.expire_due(at(50)).await.unwrap(), 0);
    let states: Vec<_> = Invitations::list(&r, g.id)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.id, i.state))
        .collect();
    assert!(states.contains(&(old.id, InvitationState::Expired)));
    assert!(states.contains(&(fresh.id, InvitationState::Pending)));
    assert!(states.contains(&(never.id, InvitationState::Pending)));
}

#[tokio::test]
async fn accepting_twice_is_refused_and_does_not_double_the_membership() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r
        .invite(g.id, &person("ada"), "member", None)
        .await
        .unwrap();
    Invitations::accept(&r, inv.id, &person("ada"), at(1))
        .await
        .unwrap();
    assert!(
        Invitations::accept(&r, inv.id, &person("ada"), at(2))
            .await
            .is_err()
    );
    assert_eq!(r.direct(g.id).await.unwrap().len(), 1);
}

#[test]
fn the_transition_function_is_the_whole_rule() {
    use InvitationState::{Accepted, Declined, Expired, Pending, Revoked};
    assert_eq!(Pending.on(Step::Accept, false).unwrap(), Accepted);
    assert_eq!(Pending.on(Step::Accept, true).unwrap(), Expired);
    assert_eq!(Pending.on(Step::Decline, false).unwrap(), Declined);
    assert_eq!(Pending.on(Step::Revoke, false).unwrap(), Revoked);
    assert_eq!(Pending.on(Step::Expire, true).unwrap(), Expired);
    for done in [Accepted, Declined, Revoked, Expired] {
        for step in [Step::Accept, Step::Decline, Step::Revoke, Step::Expire] {
            assert!(done.on(step, false).is_err(), "{done:?} is final");
        }
    }
}

#[tokio::test]
async fn an_invitation_to_an_unknown_group_or_without_invitee_is_refused() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    assert!(
        r.invite(fs_objects::ObjectId::new(), "a", "", None)
            .await
            .is_err()
    );
    assert!(r.invite(g.id, " ", "", None).await.is_err());
}

#[tokio::test]
async fn an_expired_invitation_answers_the_same_before_and_after_a_sweep() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let (before, after) = (
        r.invite(g.id, &person("ada"), "", Some(at(10)))
            .await
            .unwrap(),
        r.invite(g.id, &person("ada"), "", Some(at(10)))
            .await
            .unwrap(),
    );
    let not_swept = Invitations::accept(&r, before.id, &person("ada"), at(20)).await;
    assert!(matches!(not_swept, Err(Error::Expired(id)) if id == before.id));
    assert_eq!(r.expire_due(at(20)).await.unwrap(), 1);
    let swept = Invitations::accept(&r, after.id, &person("ada"), at(20)).await;
    assert!(matches!(swept, Err(Error::Expired(id)) if id == after.id));
    let wrong = Invitations::accept(&r, after.id, &person("bo"), at(20)).await;
    assert!(matches!(wrong, Err(Error::NotInvitee(_))), "{wrong:?}");
}

#[tokio::test]
async fn a_mail_invitation_cannot_be_accepted_by_saying_the_address() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r.invite(g.id, "ada@example.org", "", None).await.unwrap();
    let said = Invitations::accept(&r, inv.id, "ada@example.org", at(1)).await;
    assert!(matches!(said, Err(Error::NotInvitee(_))), "{said:?}");
    assert!(!r.is_member(g.id, "ada@example.org").await.unwrap());
    r.accept_proved(inv.id, "ada@example.org", "ada@example.org", at(1))
        .await
        .unwrap();
}
