//! Findings of the fourth review, as regressions. The structure that holds them is tested in
//! `mirror_property.rs`: no agent or unknown person is in the closure of an administrative group
//! after any prefix of the writes of a run.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{Agents, FakeDirectory, ME, Recording, link, person, setup};
use fs_groups_core::{Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_store::{ObjectId, Store};

const BOT: &str = "https://example.org/agents/bot";

async fn world(agents: &[&str]) -> (GroupRepo, Arc<dyn Store>, Arc<Agents>) {
    let (r, store) = setup().await;
    let list = Agents::with(agents);
    let r = r
        .with_ai_marking(list.clone())
        .with_admin_roles(["owner"])
        .unwrap();
    (r, store, list)
}

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn put(r: &GroupRepo, g: &Group, who: &str) {
    let who = if who.starts_with("http") {
        who.to_string()
    } else {
        person(who)
    };
    r.add(g.id, Member::Person(who), "").await.unwrap();
}

async fn admin(r: &GroupRepo, g: &Group) {
    let target = r.create(&format!("target {}", g.name), "t").await.unwrap();
    r.grant(target.id, Member::Group(g.id), Right::Manage)
        .await
        .unwrap();
}

fn closure<F: Fn(ObjectId) -> Option<fs_groups_core::DirectoryGroup>>(
    get: F,
    id: ObjectId,
) -> BTreeSet<String> {
    let (mut seen, mut todo, mut out) = (BTreeSet::new(), vec![id], BTreeSet::new());
    while let Some(cur) = todo.pop() {
        if let Some(g) = get(cur).filter(|_| seen.insert(cur)) {
            out.extend(g.members);
            todo.extend(g.nested);
        }
    }
    out
}

fn closure_in(dir: &FakeDirectory, id: ObjectId) -> BTreeSet<String> {
    closure(|i| dir.get(i), id)
}

/// A readable group is turned into a corrupt one by this; the id stays.
async fn corrupt_membership(store: &Arc<dyn Store>, id: ObjectId, key: &str) {
    let mut o = store.get(id).await.unwrap().unwrap();
    o.index.remove(key);
    store.put(o, ME).await.unwrap();
}

#[tokio::test]
async fn blocker_1_a_retained_edge_is_checked_against_the_state_after_the_run() {
    let (r, store, agents) = world(&[]).await;
    let (h, c) = (
        mk(&r, "h", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    put(&r, &c, "ada").await;
    let in_h = r
        .add(h.id, Member::Person(person("pat")), "")
        .await
        .unwrap();
    link(&r, c.id, h.id).await;
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().nested, [c.id]);
    corrupt_membership(&store, in_h.id, "member_kind").await; // something unreadable touches h
    agents.0.lock().unwrap().push(BOT.to_string());
    put(&r, &c, BOT).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!closure_in(&dir, h.id).contains(BOT), "{:?}", dir.get(h.id));
    if closure_in(&dir, h.id).contains(BOT) {
        assert!(report.left_out.is_empty());
    }
}

#[tokio::test]
async fn blocker_2_a_wanted_edge_does_not_let_an_agent_marked_later_stay() {
    let (r, store, agents) = world(&[]).await;
    let (h, m, u) = (
        mk(&r, "h", RightFlow::Up).await,
        mk(&r, "m", RightFlow::Up).await,
        mk(&r, "u", RightFlow::Up).await,
    );
    put(&r, &u, "ada").await;
    put(&r, &u, BOT).await;
    link(&r, m.id, h.id).await;
    link(&r, u.id, m.id).await;
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    // the agent is marked later, and u cannot be read any more
    agents.0.lock().unwrap().push(BOT.to_string());
    let mut o = store.get(u.id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("name");
    store.put(o, ME).await.unwrap();
    // an unrelated membership change makes the sync write
    put(&r, &h, "pat").await;
    r.sync_to(&dir).await.unwrap();
    assert!(!closure_in(&dir, h.id).contains(BOT), "{:?}", dir.get(h.id));
}

#[tokio::test]
async fn blocker_3_a_refused_parent_holds_back_a_child_that_loses_one_and_gains_an_agent() {
    let (r, _, agents) = world(&[]).await;
    let (h, c) = (
        mk(&r, "h", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    let ada = r
        .add(c.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    link(&r, c.id, h.id).await;
    admin(&r, &h).await;
    let rec = Recording::default();
    r.sync_to(&rec).await.unwrap();
    assert_eq!(rec.groups.lock().unwrap()[&h.id].nested, [c.id]);
    agents.0.lock().unwrap().push(BOT.to_string());
    r.remove(ada.id).await.unwrap();
    put(&r, &c, BOT).await;
    rec.refuse.lock().unwrap().insert(h.id);
    rec.start();
    r.sync_to(&rec).await.unwrap();
    for (i, state) in rec.states.lock().unwrap().iter().enumerate() {
        let seen = closure(|id| state.get(&id).cloned(), h.id);
        assert!(!seen.contains(BOT), "state {i}: {seen:?}");
    }
}

#[tokio::test]
async fn blocker_4_a_corrupt_parent_entry_keeps_the_person_in_the_parent_and_is_reported() {
    for bad in [
        serde_json::json!(["not-a-uuid"]),
        serde_json::json!([123]),
        { serde_json::json!([ObjectId::new().to_string()]) },
    ] {
        let (r, store, _) = world(&[]).await;
        let (p, c) = (
            mk(&r, "p", RightFlow::None).await,
            mk(&r, "c", RightFlow::None).await,
        );
        put(&r, &c, "ada").await;
        link(&r, c.id, p.id).await;
        let dir = FakeDirectory::default();
        r.sync_to(&dir).await.unwrap();
        assert_eq!(dir.get(p.id).unwrap().members, [person("ada")]);
        let mut o = store.get(c.id).await.unwrap().unwrap();
        o.content["fs:parents"] = bad.clone();
        store.put(o, ME).await.unwrap();
        for _ in 0..2 {
            let report = r.sync_to(&dir).await.unwrap();
            assert_eq!(dir.get(p.id).unwrap().members, [person("ada")], "{bad}");
            assert!(!report.unreadable.is_empty(), "{bad}");
            assert!(!report.is_quiet(), "{bad}");
        }
    }
}

#[tokio::test]
async fn should_5_without_an_answer_ordinary_groups_lose_nobody_to_a_corrupt_delegation() {
    let (r, store) = setup().await;
    let (g, h) = (
        mk(&r, "g", RightFlow::None).await,
        mk(&r, "h", RightFlow::None).await,
    );
    for n in 0..6 {
        put(&r, &g, &format!("p{n}")).await;
    }
    let target = r.create("target", "t").await.unwrap();
    let d = r
        .grant(target.id, Member::Group(h.id), Right::Manage)
        .await
        .unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    // the holder of the delegation can no longer be told, in the index and in the content
    let mut o = store.get(d.id).await.unwrap().unwrap();
    o.index.remove("holder_kind");
    o.index.remove("holder");
    o.content["subject"] = "garbage".into();
    store.put(o, ME).await.unwrap();
    // a host that cannot tell anybody apart
    let silent = r.clone().with_ai_marking(Arc::new(common::NoAnswer));
    let report = silent.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(g.id).unwrap().members.len(),
        6,
        "an ordinary group loses nobody"
    );
    assert!(!report.unreadable.is_empty());
    assert!(
        report.unchecked.iter().any(|(id, _)| *id == g.id),
        "but it is reported"
    );
}
