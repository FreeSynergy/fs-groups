//! Findings of the third review: unreadable data must never open the admin rule, a stuck object must
//! not freeze revocations elsewhere, the order of writes, text shapes, canonical spelling.

mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{Agents, FakeDirectory, ME, link, person, setup};
use fs_groups_core::{AiMarking, Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_store::{ObjectId, Store};

const BOT: &str = "https://example.org/agents/bot";

async fn world(agents: &[&str]) -> (GroupRepo, Arc<dyn Store>, Arc<Agents>) {
    let (r, store) = setup().await;
    let list = Agents::with(agents);
    let r = r.with_ai_marking(list.clone()).with_admin_roles(["owner"]).unwrap();
    (r, store, list)
}

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn put(r: &GroupRepo, g: &Group, who: &str) {
    let who = if who.starts_with("http") { who.to_string() } else { person(who) };
    r.add(g.id, Member::Person(who), "").await.unwrap();
}

async fn admin(r: &GroupRepo, g: &Group) {
    let target = r.create(&format!("target {}", g.name), "t").await.unwrap();
    r.grant(target.id, Member::Group(g.id), Right::Manage).await.unwrap();
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
    let d = r.grant(target.id, Member::Group(h.id), Right::Manage).await.unwrap();
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
    let m = r.add(h.id, Member::Person(person("ada")), "owner").await.unwrap();
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
async fn an_agent_marked_later_leaves_an_administrative_group_even_when_its_own_membership_is_corrupt() {
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
    let ada = r.add(other.id, Member::Person(person("ada")), "").await.unwrap();
    put(&r, &other, "bo").await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    // a person is removed, an agent is hand-added to the administrative group, an unrelated group breaks
    r.remove(ada.id).await.unwrap();
    dir.groups.lock().unwrap().get_mut(&h.id).unwrap().members.push(BOT.to_string());
    corrupt_group(&store, stuck.id).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!report.is_quiet());
    assert_eq!(dir.get(other.id).unwrap().members, [person("bo")], "the revocation goes on");
    assert!(dir.get(h.id).unwrap().members.is_empty(), "so does the agent's removal");
}

// ---- order ----

#[tokio::test]
async fn the_agent_is_not_applied_to_a_child_before_its_administrative_parent_lets_go() {
    let (r, _, agents) = world(&[]).await;
    let (h, c) = (mk(&r, "h", RightFlow::Up).await, mk(&r, "c", RightFlow::Up).await);
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
    assert!(!closure(&dir, h.id).contains(BOT), "never in the parent's closure, not even for a moment");
    assert!(report.failed.iter().any(|(id, _)| *id == h.id));
    // the refusal ends: the next run completes
    *dir.refuse.lock().unwrap() = None;
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(c.id).unwrap().members.contains(&BOT.to_string()));
    assert!(!closure(&dir, h.id).contains(BOT));
}

// ---- text ----

#[tokio::test]
async fn invisible_and_look_alike_text_is_refused_by_shape() {
    let (r, _, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    for bad in [
        "ada\u{2800}", "ada\u{8e2}", "ada\u{13430}", "ada\u{1bca0}", "ada\u{110bd}", "ada\u{1d173}",
        "ada smith", "ada\u{a0}x",
    ] {
        assert!(r.add(g.id, Member::Person(bad.into()), "").await.is_err(), "person {bad:?}");
        assert!(r.add(g.id, Member::Person(person("ada")), bad).await.is_err(), "role {bad:?}");
    }
    // a Cyrillic o in an admin role
    assert!(r.add(g.id, Member::Person(person("ada")), "\u{43e}wner").await.is_err());
    assert!(r.add(g.id, Member::Person(person("ada")), "Owner").await.is_ok());
    assert!(r.add(g.id, Member::Person("https://example.org/people/Zoë".into()), "").await.is_ok());
}

/// Records what it is asked.
struct Recorder(Mutex<Vec<String>>);

#[async_trait]
impl AiMarking for Recorder {
    async fn is_ai(&self, person: &str) -> fs_groups_core::Result<Option<bool>> {
        self.0.lock().unwrap().push(person.to_string());
        Ok(Some(false))
    }
}

#[tokio::test]
async fn the_host_is_asked_with_scheme_and_host_lowercased_and_nothing_else_changed() {
    let (r, _) = setup().await;
    let rec = Arc::new(Recorder(Mutex::new(vec![])));
    let r = r.with_ai_marking(rec.clone());
    let g = r.create("g", "t").await.unwrap();
    for id in [
        "HTTPS://ADA@IDM.Example.ORG:8443/Ada/?u=Ada#Frag",
        "https://example.org/people/ada/",
    ] {
        r.grant(g.id, Member::Person(id.into()), Right::Manage).await.unwrap();
    }
    assert_eq!(
        *rec.0.lock().unwrap(),
        [
            "https://ADA@idm.example.org:8443/Ada/?u=Ada#Frag",
            "https://example.org/people/ada/"
        ]
    );
}

// ---- minor ----

struct Broken;

#[async_trait]
impl AiMarking for Broken {
    async fn is_ai(&self, _: &str) -> fs_groups_core::Result<Option<bool>> {
        Err(fs_groups_core::Error::Directory("host down".into()))
    }
}

#[tokio::test]
async fn reads_answer_while_the_checker_fails_and_show_not_in_force() {
    let (r, _, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    r.add(g.id, Member::Person(person("ada")), "owner").await.unwrap();
    r.grant(g.id, Member::Person(person("ada")), Right::Manage).await.unwrap();
    let down = r.clone().with_ai_marking(Arc::new(Broken));
    assert!(!down.direct(g.id).await.unwrap()[0].in_force);
    assert!(!down.delegations(g.id).await.unwrap()[0].in_force);
    assert_eq!(down.effective(g.id).await.unwrap().len(), 1);
    down.add(g.id, Member::Person(person("bo")), "").await.unwrap();
    down.grant(g.id, Member::Person(person("bo")), Right::Read).await.unwrap();
}

#[tokio::test]
async fn a_group_whose_type_changed_in_the_store_is_not_removed_from_the_directory() {
    let (r, store, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut o = store.get(g.id).await.unwrap().unwrap();
    o.envelope.object_type = "Note".into();
    store.put(o, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).is_some());
    assert_eq!(report.unreadable, [g.id]);
    assert!(!report.is_quiet());
}
