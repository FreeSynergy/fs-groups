//! ADR 0056 in the mirror: an agent, or somebody not known to be human, is never written into a
//! directory group that carries an administrative right (`Manage` on anything, or a member with an
//! admin role), however they would get there. The rest of the flat list is unchanged.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{Agents, FakeDirectory, ME, NoAnswer, link, person, setup};
use fs_groups_core::{Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_store::ObjectId;

const BOT: &str = "https://example.org/agents/bot";

async fn world(agents: &[&str]) -> (GroupRepo, Arc<Agents>) {
    let (r, _) = setup().await;
    let list = Agents::with(agents);
    (
        r.with_ai_marking(list.clone())
            .with_admin_roles(["owner"])
            .unwrap(),
        list,
    )
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

/// Make `g` carry an administrative right: it holds `Manage` on a group of its own.
async fn make_admin(r: &GroupRepo, g: &Group) {
    let target = r
        .create(&format!("target of {}", g.name), "t")
        .await
        .unwrap();
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

#[tokio::test]
async fn a_direct_agent_is_left_out_of_a_group_holding_manage_and_named() {
    let (r, _) = world(&[BOT]).await;
    let g = mk(&r, "admins", RightFlow::None).await;
    let other = mk(&r, "others", RightFlow::None).await;
    for grp in [&g, &other] {
        put(&r, grp, "ada").await;
        put(&r, grp, BOT).await;
    }
    make_admin(&r, &g).await;
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
    assert_eq!(report.left_out, [(g.id, vec![BOT.to_string()])]);
    assert!(
        dir.get(other.id)
            .unwrap()
            .members
            .contains(&BOT.to_string()),
        "no admin right there"
    );
}

#[tokio::test]
async fn a_group_with_an_admin_role_member_is_administrative_too() {
    let (r, _) = world(&[BOT]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    r.add(g.id, Member::Person(person("ada")), "Owner")
        .await
        .unwrap();
    put(&r, &g, BOT).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
}

#[tokio::test]
async fn a_flat_child_does_not_bring_an_agent_into_an_administrative_parent() {
    let (r, _) = world(&[BOT]).await;
    let (p, c) = (
        mk(&r, "p", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    put(&r, &p, "pat").await;
    put(&r, &c, "ada").await;
    put(&r, &c, BOT).await;
    link(&r, c.id, p.id).await;
    make_admin(&r, &p).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(p.id).unwrap().members,
        [person("ada"), person("pat")]
    );
    assert!(dir.get(c.id).unwrap().members.contains(&BOT.to_string()));
}

#[tokio::test]
async fn a_nested_edge_that_would_bring_an_agent_is_written_flat_without_them() {
    let (r, _) = world(&[BOT]).await;
    let (p, c) = (
        mk(&r, "p", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    put(&r, &c, "ada").await;
    put(&r, &c, BOT).await;
    link(&r, c.id, p.id).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(p.id).unwrap().nested,
        [c.id],
        "nothing administrative yet: nested"
    );
    make_admin(&r, &p).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(dir.get(p.id).unwrap().nested.is_empty(), "healed to flat");
    assert_eq!(dir.get(p.id).unwrap().members, [person("ada")]);
    assert!(!closure(&dir, p.id).contains(&BOT.to_string()));
    assert_eq!(report.left_out, [(p.id, vec![BOT.to_string()])]);
}

#[tokio::test]
async fn a_clean_child_stays_nested_in_an_administrative_parent() {
    let (r, _) = world(&[BOT]).await;
    let (p, c) = (
        mk(&r, "p", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    put(&r, &c, "ada").await;
    link(&r, c.id, p.id).await;
    make_admin(&r, &p).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(p.id).unwrap().nested, [c.id]);
}

#[tokio::test]
async fn a_chain_keeps_an_agent_out_of_the_whole_closure() {
    let (r, _) = world(&[BOT]).await;
    // q (administrative) <- p <- c, all passing up; the agent is in c, p itself is not administrative
    let q = mk(&r, "q", RightFlow::Up).await;
    let p = mk(&r, "p", RightFlow::Up).await;
    let c = mk(&r, "c", RightFlow::Up).await;
    put(&r, &q, "quinn").await;
    put(&r, &p, "pat").await;
    put(&r, &c, "ada").await;
    put(&r, &c, BOT).await;
    link(&r, p.id, q.id).await;
    link(&r, c.id, p.id).await;
    make_admin(&r, &q).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(p.id).unwrap().nested,
        [c.id],
        "p is not administrative"
    );
    assert!(
        dir.get(q.id).unwrap().nested.is_empty(),
        "p's closure holds the agent"
    );
    assert_eq!(
        closure(&dir, q.id),
        [person("ada"), person("pat"), person("quinn")].into()
    );
}

#[tokio::test]
async fn a_delegation_added_later_removes_the_agent_at_the_next_sync() {
    let (r, _) = world(&[BOT]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    put(&r, &g, "ada").await;
    put(&r, &g, BOT).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).unwrap().members.contains(&BOT.to_string()));
    make_admin(&r, &g).await;
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
}

#[tokio::test]
async fn a_marking_that_appears_later_removes_the_person_at_the_next_sync() {
    let (r, agents) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    put(&r, &g, "ada").await;
    make_admin(&r, &g).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
    agents.0.lock().unwrap().push(person("ada"));
    let report = r.sync_to(&dir).await.unwrap();
    assert!(dir.get(g.id).unwrap().members.is_empty());
    assert_eq!(report.left_out, [(g.id, vec![person("ada")])]);
}

#[tokio::test]
async fn unknown_counts_as_an_agent_and_so_does_no_checker() {
    for label in ["no answer", "no checker"] {
        let (r, store) = setup().await;
        let g = mk(&r, "g", RightFlow::None).await;
        let c = mk(&r, "c", RightFlow::None).await;
        put(&r, &g, "ada").await;
        put(&r, &c, "bo").await;
        link(&r, c.id, g.id).await;
        make_admin(&r, &g).await;
        // the same data, seen by a repository that cannot tell who is a human
        let plain = GroupRepo::new(store, ME).with_instance("example.org");
        let repo = if label == "no answer" {
            plain.with_ai_marking(Arc::new(NoAnswer))
        } else {
            plain
        };
        let dir = FakeDirectory::default();
        let report = repo.sync_to(&dir).await.unwrap();
        assert!(dir.get(g.id).unwrap().members.is_empty(), "{label}");
        assert_eq!(
            report.left_out,
            [(g.id, vec![person("ada"), person("bo")])],
            "{label}: named in the report"
        );
        assert_eq!(
            dir.get(c.id).unwrap().members,
            [person("bo")],
            "{label}: c is not administrative"
        );
    }
}

#[tokio::test]
async fn nobody_is_asked_while_no_group_is_administrative() {
    let (r, _) = setup().await;
    let unwired = GroupRepo::new(setup().await.1, ME).with_instance("example.org");
    drop(r);
    let g = mk(&unwired, "g", RightFlow::None).await;
    put(&unwired, &g, "ada").await;
    let dir = FakeDirectory::default();
    let report = unwired.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(g.id).unwrap().members, [person("ada")]);
    assert!(report.left_out.is_empty());
}
