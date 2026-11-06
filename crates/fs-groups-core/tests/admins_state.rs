//! Admin entries that are no longer in force, and the configuration around admins (ADR 0056).

mod common;

use std::sync::Arc;

use common::{Agents, NoAnswer, person, setup};
use fs_groups_core::{GroupRepo, Member, Right};

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
async fn an_entry_whose_holder_is_no_longer_known_human_is_marked_not_in_force() {
    let (r, agents) = repo_with(&[]).await;
    let g = r.create("g", "t").await.unwrap();
    r.add(g.id, Member::Person(person("ada")), "owner")
        .await
        .unwrap();
    r.add(g.id, Member::Person(person("ada")), "member")
        .await
        .unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage)
        .await
        .unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    assert!(r.direct(g.id).await.unwrap().iter().all(|m| m.in_force));
    assert!(
        r.delegations(g.id)
            .await
            .unwrap()
            .iter()
            .all(|d| d.in_force)
    );
    agents.0.lock().unwrap().push(person("ada"));
    let held = r.direct(g.id).await.unwrap();
    assert!(!held.iter().find(|m| m.role == "owner").unwrap().in_force);
    assert!(held.iter().find(|m| m.role == "member").unwrap().in_force);
    let rights = r.delegations(g.id).await.unwrap();
    assert!(
        !rights
            .iter()
            .find(|d| d.right == Right::Manage)
            .unwrap()
            .in_force
    );
    assert!(
        rights
            .iter()
            .find(|d| d.right == Right::Read)
            .unwrap()
            .in_force
    );
    // and no answer at all is the same
    let silent = r.clone().with_ai_marking(Arc::new(NoAnswer));
    assert!(
        !silent
            .delegations(g.id)
            .await
            .unwrap()
            .iter()
            .find(|d| d.right == Right::Manage)
            .unwrap()
            .in_force
    );
}

#[tokio::test]
async fn an_admin_role_only_bars_agents_it_grants_no_right() {
    let (r, _) = repo_with(&[]).await;
    let g = r.create("g", "t").await.unwrap();
    r.add(g.id, Member::Person(person("ada")), "owner")
        .await
        .unwrap();
    assert!(!r.may_do(g.id, &person("ada"), Right::Manage).await.unwrap());
}

#[tokio::test]
async fn bad_configuration_is_refused() {
    let (r, _) = repo_with(&[]).await;
    for bad in ["", "  ", " boss", "boss ", "boss\u{200b}", "bo\u{0}ss"] {
        assert!(r.clone().with_installer(bad).is_err(), "installer {bad:?}");
    }
    for bad in ["", "  ", "ow\u{200b}ner", "ow\u{7}ner"] {
        assert!(r.clone().with_admin_roles([bad]).is_err(), "role {bad:?}");
    }
}

#[tokio::test]
async fn the_installer_gets_nothing_on_a_group_that_is_on_a_loop() {
    let (r, store) = setup().await;
    let boss = person("installer");
    let r = r.with_installer(boss.clone()).unwrap();
    let (a, b) = (
        r.create("a", "t").await.unwrap(),
        r.create("b", "t").await.unwrap(),
    );
    common::link(&r, b.id, a.id).await;
    common::force_parent(&store, a.id, b.id).await;
    assert!(!r.may_do(a.id, &boss, Right::Manage).await.unwrap());
}
