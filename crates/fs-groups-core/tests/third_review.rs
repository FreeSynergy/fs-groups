//! Findings of the third review: unreadable data must never open the admin rule, a stuck object must
//! not freeze revocations elsewhere, the order of writes, text shapes, canonical spelling.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{Agents, FakeDirectory, ME, link, person, setup};
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

fn closure(dir: &FakeDirectory, id: ObjectId) -> BTreeSet<String> {
    let (mut seen, mut todo, mut out) = (BTreeSet::new(), vec![id], BTreeSet::new());
    while let Some(cur) = todo.pop() {
        if let Some(g) = dir.get(cur).filter(|_| seen.insert(cur)) {
            out.extend(g.members);
            todo.extend(g.nested);
        }
    }
    out
}

/// A group that cannot be read as a group but still says which parents it has.
async fn corrupt_group(store: &Arc<dyn Store>, id: ObjectId) {
    let mut o = store.get(id).await.unwrap().unwrap();
    o.content.as_object_mut().unwrap().remove("name");
    store.put(o, ME).await.unwrap();
}

#[tokio::test]
async fn an_unrelated_unreadable_group_does_not_let_an_agent_into_an_administrative_closure() {
    let (r, store, agents) = world(&[]).await;
    let (h, c, x) = (
        mk(&r, "h", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
        mk(&r, "x", RightFlow::None).await,
    );
    put(&r, &c, "ada").await;
    link(&r, c.id, h.id).await;
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().nested, [c.id]);
    corrupt_group(&store, x.id).await;
    agents.0.lock().unwrap().push(BOT.to_string());
    put(&r, &c, BOT).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!closure(&dir, h.id).contains(BOT), "{:?}", dir.get(h.id));
    assert!(dir.get(h.id).unwrap().nested.is_empty());
    assert!(!report.is_quiet());
}

#[tokio::test]
async fn an_unreadable_delegation_does_not_remove_administrative_status() {
    let (r, store, _) = world(&[BOT]).await;
    let h = mk(&r, "h", RightFlow::None).await;
    put(&r, &h, "ada").await;
    let target = r.create("target", "t").await.unwrap();
    let d = r
        .grant(target.id, Member::Group(h.id), Right::Manage)
        .await
        .unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(d.id).await.unwrap().unwrap();
    o.index.remove("holder");
    store.put(o, ME).await.unwrap();
    put(&r, &h, BOT).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().members, [person("ada")]);
    assert_eq!(report.unreadable, [d.id]);
    assert!(!r.sync_to(&dir).await.unwrap().is_quiet());
}

#[tokio::test]
async fn an_unreadable_admin_role_membership_holds_still_and_adds_no_agent() {
    let (r, store, _) = world(&[BOT]).await;
    let h = mk(&r, "h", RightFlow::None).await;
    let m = r
        .add(h.id, Member::Person(person("ada")), "owner")
        .await
        .unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(m.id).await.unwrap().unwrap();
    o.index.remove("member");
    store.put(o, ME).await.unwrap();
    put(&r, &h, BOT).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!dir.get(h.id).unwrap().members.contains(&BOT.to_string()));
    assert_eq!(report.unreadable, [m.id]);
}

// ---- holding still is scoped to what the unreadable object touches ----

#[tokio::test]
async fn an_agent_marked_later_leaves_an_administrative_group_even_when_its_own_membership_is_corrupt()
 {
    let (r, store, agents) = world(&[]).await;
    let h = mk(&r, "h", RightFlow::None).await;
    put(&r, &h, "ada").await;
    let m = r.add(h.id, Member::Person(BOT.into()), "").await.unwrap();
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(h.id).unwrap().members.contains(&BOT.to_string()));
    agents.0.lock().unwrap().push(BOT.to_string());
    let mut o = store.get(m.id).await.unwrap().unwrap();
    o.index.remove("member_kind");
    store.put(o, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().members, [person("ada")]);
    assert_eq!(report.left_out, [(h.id, vec![BOT.to_string()])]);
}

#[tokio::test]
async fn one_stuck_object_does_not_freeze_revocations_elsewhere() {
    let (r, store, _) = world(&[BOT]).await;
    let (h, other, stuck) = (
        mk(&r, "h", RightFlow::None).await,
        mk(&r, "other", RightFlow::None).await,
        mk(&r, "stuck", RightFlow::None).await,
    );
    admin(&r, &h).await;
    let ada = r
        .add(other.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    put(&r, &other, "bo").await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    // a person is removed, an agent is hand-added to the administrative group, an unrelated group breaks
    r.remove(ada.id).await.unwrap();
    dir.groups
        .lock()
        .unwrap()
        .get_mut(&h.id)
        .unwrap()
        .members
        .push(BOT.to_string());
    corrupt_group(&store, stuck.id).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!report.is_quiet());
    assert_eq!(
        dir.get(other.id).unwrap().members,
        [person("bo")],
        "the revocation goes on"
    );
    assert!(
        dir.get(h.id).unwrap().members.is_empty(),
        "so does the agent's removal"
    );
}

// ---- order ----

#[tokio::test]
async fn the_agent_is_not_applied_to_a_child_before_its_administrative_parent_lets_go() {
    let (r, _, agents) = world(&[]).await;
    let (h, c) = (
        mk(&r, "h", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    put(&r, &c, "ada").await;
    link(&r, c.id, h.id).await;
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().nested, [c.id]);
    agents.0.lock().unwrap().push(BOT.to_string());
    put(&r, &c, BOT).await;
    *dir.refuse.lock().unwrap() = Some(h.id); // the parent cannot be updated
    let report = r.sync_to(&dir).await.unwrap();
    assert!(
        !closure(&dir, h.id).contains(BOT),
        "never in the parent's closure, not even for a moment"
    );
    assert!(report.failed.iter().any(|(id, _)| *id == h.id));
    // the refusal ends: the next run completes
    *dir.refuse.lock().unwrap() = None;
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(c.id).unwrap().members.contains(&BOT.to_string()));
    assert!(!closure(&dir, h.id).contains(BOT));
}

#[tokio::test]
async fn a_flat_child_with_a_corrupt_membership_still_keeps_the_agent_out_of_the_parent() {
    let (r, store, agents) = world(&[]).await;
    let (h, c) = (
        mk(&r, "h", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    put(&r, &h, "ada").await;
    let m = r.add(c.id, Member::Person(BOT.into()), "").await.unwrap();
    link(&r, c.id, h.id).await;
    admin(&r, &h).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(h.id).unwrap().members.contains(&BOT.to_string()));
    agents.0.lock().unwrap().push(BOT.to_string());
    let mut o = store.get(m.id).await.unwrap().unwrap();
    o.index.remove("member_kind");
    store.put(o, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(h.id).unwrap().members, [person("ada")]);
    // the group with the corrupt membership counts as administrative too (fail closed)
    let named: BTreeSet<_> = report.left_out.iter().map(|(id, _)| *id).collect();
    assert_eq!(named, [h.id, c.id].into());
}
