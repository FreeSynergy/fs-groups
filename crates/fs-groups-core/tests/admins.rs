//! ADR 0056: an agent never becomes admin, and the installer is the top admin (provisional).
//! Admin means `Right::Manage` or a role the host names. Whether a member is an agent is the host's
//! answer (`AiMarking`); without an answer the grant is refused. Nothing here verifies who calls.

mod common;

use std::sync::Arc;

use common::{Agents, ME, NoAnswer, person, setup};
use fs_groups_core::{Error, GroupRepo, Member, Right};

const BOT: &str = "https://example.org/agents/bot";

async fn repo_with(agents: &[&str]) -> (GroupRepo, Arc<Agents>) {
    let (r, _) = setup().await;
    let list = Agents::with(agents);
    (
        r.with_ai_marking(list.clone())
            .with_admin_roles(["owner"])
            .unwrap(),
        list,
    )
}

#[tokio::test]
async fn an_agent_cannot_be_granted_manage() {
    let (r, _) = repo_with(&[BOT]).await;
    let g = r.create("g", "t").await.unwrap();
    let err = r
        .grant(g.id, Member::Person(BOT.into()), Right::Manage)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::AgentNotAdmin(_)), "{err}");
    assert!(r.delegations(g.id).await.unwrap().is_empty());
    // other rights are not admin, and a human may be admin
    r.grant(g.id, Member::Person(BOT.into()), Right::Read)
        .await
        .unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_agent_cannot_get_an_admin_role() {
    let (r, _) = repo_with(&[BOT]).await;
    let g = r.create("g", "t").await.unwrap();
    let err = r
        .add(g.id, Member::Person(BOT.into()), "owner")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::AgentNotAdmin(_)), "{err}");
    r.add(g.id, Member::Person(BOT.into()), "member")
        .await
        .unwrap();
    r.add(g.id, Member::Person(person("ada")), "owner")
        .await
        .unwrap();
    assert_eq!(r.direct(g.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn accepting_an_admin_role_is_checked_too_and_the_invitation_stays() {
    let (r, _) = repo_with(&[BOT]).await;
    let g = r.create("g", "t").await.unwrap();
    let inv = r.invite(g.id, BOT, "owner", None).await.unwrap();
    let err = r.accept(inv.id, BOT, common::at(1)).await.unwrap_err();
    assert!(matches!(err, Error::AgentNotAdmin(_)));
    assert!(r.direct(g.id).await.unwrap().is_empty());
    assert_eq!(
        r.invitations(g.id).await.unwrap()[0].state,
        fs_groups_core::InvitationState::Pending
    );
}

#[tokio::test]
async fn no_answer_means_refuse() {
    // no checker wired at all
    let (wired, store) = setup().await;
    let unwired = GroupRepo::new(store, ME).with_instance("example.org");
    let g = unwired.create("g", "t").await.unwrap();
    let err = unwired
        .grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::AiStatusUnknown(_)), "{err}");
    drop(wired);
    // a checker that cannot tell
    let silent = unwired
        .with_ai_marking(Arc::new(NoAnswer))
        .with_admin_roles(["owner"])
        .unwrap();
    assert!(matches!(
        silent
            .grant(g.id, Member::Person(person("ada")), Right::Manage)
            .await,
        Err(Error::AiStatusUnknown(_))
    ));
    assert!(matches!(
        silent
            .add(g.id, Member::Person(person("ada")), "owner")
            .await,
        Err(Error::AiStatusUnknown(_))
    ));
    // nothing admin-like is needed for the rest
    silent
        .add(g.id, Member::Person(person("ada")), "member")
        .await
        .unwrap();
    silent
        .grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
}

#[tokio::test]
async fn through_a_holder_group_an_agent_is_left_out_of_manage_only() {
    let (r, agents) = repo_with(&[BOT]).await;
    let (target, team) = (
        r.create("target", "t").await.unwrap(),
        r.create("team", "t").await.unwrap(),
    );
    r.add(team.id, Member::Person(BOT.into()), "")
        .await
        .unwrap();
    r.add(team.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    // PROVISIONAL: a group that contains an agent may hold manage; the agent is excluded when asked
    r.grant(target.id, Member::Group(team.id), Right::Manage)
        .await
        .unwrap();
    assert!(
        r.may_do(target.id, &person("ada"), Right::AddMembers)
            .await
            .unwrap()
    );
    assert!(!r.may_do(target.id, BOT, Right::AddMembers).await.unwrap());
    assert!(!r.may_do(target.id, BOT, Right::Manage).await.unwrap());
    // a right below manage through a group is fine for an agent
    r.grant(target.id, Member::Group(team.id), Right::Read)
        .await
        .unwrap();
    assert!(r.may_do(target.id, BOT, Right::Read).await.unwrap());
    // marked later: the answer changes with the marking
    agents.0.lock().unwrap().push(person("ada"));
    assert!(
        !r.may_do(target.id, &person("ada"), Right::Manage)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn an_unknown_answer_or_a_failure_never_leaves_manage_in_force() {
    let (r, _) = repo_with(&[]).await;
    let g = r.create("g", "t").await.unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    assert!(r.may_do(g.id, &person("ada"), Right::Manage).await.unwrap());
    let silent = r.clone().with_ai_marking(Arc::new(NoAnswer));
    assert!(
        !silent
            .may_do(g.id, &person("ada"), Right::Manage)
            .await
            .unwrap()
    );
    assert!(
        silent
            .may_do(g.id, &person("ada"), Right::Read)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn the_installer_is_the_top_admin_unless_marked_as_an_agent() {
    let (r, agents) = repo_with(&[]).await;
    let boss = person("installer");
    let r = r.with_installer(boss.clone()).unwrap();
    let g = r.create("g", "t").await.unwrap();
    assert_eq!(r.installer(), Some(boss.as_str()));
    assert!(
        r.may_do(g.id, &boss, Right::Manage).await.unwrap(),
        "without any delegation"
    );
    assert!(!r.may_do(g.id, &person("ada"), Right::Read).await.unwrap());
    // appointing further admins is an ordinary grant of manage
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    assert!(r.may_do(g.id, &person("ada"), Right::Manage).await.unwrap());
    // never an agent, not even if the configuration names one
    agents.0.lock().unwrap().push(boss.clone());
    assert!(!r.may_do(g.id, &boss, Right::Manage).await.unwrap());
}

#[tokio::test]
async fn an_agent_cannot_answer_for_a_parent() {
    let (r, store) = {
        let (r, store) = setup().await;
        (r, store)
    };
    let bot = GroupRepo::new(store, BOT)
        .with_instance("example.org")
        .with_ai_marking(Agents::with(&[BOT]));
    let (c, p) = (
        bot.create("c", "t").await.unwrap(),
        bot.create("p", "t").await.unwrap(),
    );
    let req = bot.request_parent(c.id, p.id).await.unwrap();
    // it created the parent, but an agent is never admin of it
    assert!(matches!(
        bot.approve_parent(req.id).await,
        Err(Error::Forbidden(_))
    ));
    drop(r);
}

struct Broken;

#[async_trait::async_trait]
impl fs_groups_core::AiMarking for Broken {
    async fn is_ai(&self, _person: &str) -> fs_groups_core::Result<Option<bool>> {
        Err(Error::Directory("the host cannot answer".into()))
    }
}

#[tokio::test]
async fn a_failing_answer_is_an_error_and_grants_nothing() {
    let (r, _) = repo_with(&[]).await;
    let g = r.create("g", "t").await.unwrap();
    let r = r.with_ai_marking(Arc::new(Broken));
    assert!(
        r.grant(g.id, Member::Person(person("ada")), Right::Manage)
            .await
            .is_err()
    );
    assert!(r.delegations(g.id).await.unwrap().is_empty());
}
