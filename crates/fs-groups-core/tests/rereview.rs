//! Findings of the second review, pinned by tests that failed before the fixes.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{Agents, FakeDirectory, ME, at, force_parent, person, repo, setup};
use fs_groups_core::{AiMarking, Error, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_store::Query;

const BOT: &str = "https://example.org/agents/bot";

// ---- role spelling ----

#[tokio::test]
async fn a_variant_of_an_admin_role_is_still_admin() {
    for configured in [" owner ", "OWNER", "Owner"] {
        let (r, _) = setup().await;
        let r = r
            .with_ai_marking(Agents::with(&[BOT]))
            .with_admin_roles([configured]);
        let g = r.create("g", "t").await.unwrap();
        for given in ["owner", "OWNER", "Owner", " owner ", "\u{ff2f}wner"] {
            let err = r.add(g.id, Member::Person(BOT.into()), given).await;
            assert!(
                matches!(err, Err(Error::AgentNotAdmin(_) | Error::Conflict(_))),
                "configured {configured:?}, given {given:?}: {err:?}"
            );
        }
        // and through an invitation
        let inv = r.invite(g.id, BOT, "Owner", None).await.unwrap();
        assert!(r.accept(inv.id, BOT, at(1)).await.is_err());
        assert!(r.direct(g.id).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn invisible_and_control_characters_are_refused_at_the_input() {
    let r = repo().await;
    let g = r.create("g", "t").await.unwrap();
    for bad in ["owner\u{200b}", "ow\u{0}ner", "own\u{feff}er", "o\u{202e}wner"] {
        assert!(r.add(g.id, Member::Person(person("ada")), bad).await.is_err(), "{bad:?}");
        assert!(r.invite(g.id, &person("ada"), bad, None).await.is_err(), "{bad:?}");
        assert!(r.add(g.id, Member::Person(bad.into()), "").await.is_err(), "{bad:?}");
        assert!(
            r.grant(g.id, Member::Person(bad.into()), Right::Read).await.is_err(),
            "{bad:?}"
        );
        assert!(r.may_do(g.id, bad, Right::Read).await.is_err(), "{bad:?}");
    }
    assert!(r.direct(g.id).await.unwrap().is_empty());
}

// ---- the person is normalised before the host is asked ----

/// What a thin wrapper over a role book does: an id it knows is an agent, everything else "human".
struct Wrapper;

#[async_trait]
impl AiMarking for Wrapper {
    async fn is_ai(&self, person: &str) -> fs_groups_core::Result<Option<bool>> {
        Ok(Some(person == BOT))
    }
}

#[tokio::test]
async fn a_spelling_of_an_agent_id_does_not_become_human() {
    let (r, _) = setup().await;
    let r = r.with_ai_marking(Arc::new(Wrapper));
    let g = r.create("g", "t").await.unwrap();
    for variant in [
        "HTTPS://EXAMPLE.ORG/agents/bot",
        "https://example.org/agents/bot/",
        " https://example.org/agents/bot ",
    ] {
        let err = r.grant(g.id, Member::Person(variant.into()), Right::Manage).await;
        assert!(matches!(err, Err(Error::AgentNotAdmin(_))), "{variant:?}: {err:?}");
    }
}

// ---- the safe reading is not widened ----

#[tokio::test]
async fn an_up_holder_does_not_reach_the_members_of_an_up_child() {
    let (r, _) = setup().await;
    let r = r.with_ai_marking(Agents::with(&[]));
    let (club, board, interns) = (
        r.create("club", "t").await.unwrap(),
        r.create("board", "t").await.unwrap(),
        r.create("interns", "t").await.unwrap(),
    );
    for g in [&board, &interns] {
        r.set_right_flow(g.id, RightFlow::Up).await.unwrap();
    }
    common::link(&r, interns.id, board.id).await;
    r.add(interns.id, Member::Person(person("intern")), "").await.unwrap();
    r.grant(club.id, Member::Group(board.id), Right::Manage).await.unwrap();
    assert!(!r.may_do(club.id, &person("intern"), Right::Manage).await.unwrap());
    // a child approved while it said none, which then says up on its own
    let late = r.create("late", "t").await.unwrap();
    common::link(&r, late.id, board.id).await;
    r.add(late.id, Member::Person(person("late")), "").await.unwrap();
    r.set_right_flow(late.id, RightFlow::Up).await.unwrap();
    assert!(!r.may_do(club.id, &person("late"), Right::Manage).await.unwrap());
}

// ---- the mirror and what it cannot read ----

#[tokio::test]
async fn an_unreadable_group_in_the_middle_takes_nobody_out_of_the_directory() {
    let (r, store) = setup().await;
    let (p, u, c) = (
        r.create("p", "t").await.unwrap(),
        r.create("u", "t").await.unwrap(),
        r.create("c", "t").await.unwrap(),
    );
    for g in [&p, &u, &c] {
        r.set_right_flow(g.id, RightFlow::Up).await.unwrap();
    }
    common::link(&r, u.id, p.id).await;
    common::link(&r, c.id, u.id).await;
    r.add(c.id, Member::Person(person("ada")), "").await.unwrap();
    r.add(p.id, Member::Person(person("pat")), "").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(u.id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("name");
    store.put(o, ME).await.unwrap();
    for _ in 0..2 {
        let report = r.sync_to(&dir).await.unwrap();
        assert_eq!(report.unreadable, [u.id]);
        assert!(!report.is_quiet(), "something cannot be read");
        assert!(dir.get(u.id).is_some() && dir.get(p.id).unwrap().members.contains(&person("pat")));
    }
    // ada is still in the directory somewhere under p
    let mut all = dir.get(p.id).unwrap().members;
    for id in [u.id, c.id] {
        all.extend(dir.get(id).unwrap().members);
    }
    assert!(all.contains(&person("ada")));
    // and the unreadable group can be deleted
    r.delete(u.id).await.unwrap();
    assert!(store.get(u.id).await.unwrap().is_none());
}

#[tokio::test]
async fn an_unreadable_membership_is_reported_and_nobody_is_removed() {
    let (r, store) = setup().await;
    let g = r.create("g", "t").await.unwrap();
    let m = r.add(g.id, Member::Person(person("ada")), "").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(m.id).await.unwrap().unwrap();
    o.index.remove("group");
    store.put(o, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.unreadable, [m.id]);
    assert!(!report.is_quiet());
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
    r.remove(m.id).await.unwrap();
}

#[tokio::test]
async fn cyclic_data_from_a_second_writer_never_reaches_the_directory_as_a_loop() {
    let (r, store) = setup().await;
    let (a, b) = (r.create("a", "t").await.unwrap(), r.create("b", "t").await.unwrap());
    for g in [&a, &b] {
        r.set_right_flow(g.id, RightFlow::Up).await.unwrap();
    }
    common::link(&r, b.id, a.id).await;
    force_parent(&store, a.id, b.id).await;
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    for id in [a.id, b.id] {
        assert!(dir.get(id).unwrap().nested.is_empty(), "no loop in the directory");
    }
    assert!(!report.is_quiet());
}

#[tokio::test]
async fn a_repeat_does_not_rewrite_the_ledger() {
    let (r, store) = setup().await;
    r.create("g", "t").await.unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let ledger = |store: &Arc<dyn fs_store::Store>| {
        let store = store.clone();
        async move {
            let q = Query::new().of_type("MirrorLedger");
            store.query(&q).await.unwrap().remove(0).envelope.updated
        }
    };
    let before = ledger(&store).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    r.sync_to(&dir).await.unwrap();
    assert_eq!(ledger(&store).await, before);
}

#[allow(dead_code)]
fn _unused(_: GroupRepo) {}
