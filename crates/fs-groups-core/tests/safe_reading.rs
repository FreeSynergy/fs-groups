//! The safe reading of up and down (provisional) and "in doubt, no" for `may_do`.

mod common;

use std::collections::BTreeMap;

use common::{faulty_repo, force_parent, person, repo, setup};
use fs_groups_core::right_flow::{reach, sources};
use fs_groups_core::{Error, Graph, Group, GroupRepo, Member, Right, RightFlow};
use fs_store::ObjectId;

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn may(r: &GroupRepo, on: &Group, who: &str) -> bool {
    r.may_do(on.id, &person(who), Right::Read).await.unwrap()
}

#[tokio::test]
async fn provisional_a_board_passing_down_does_not_reach_an_intern_in_a_child() {
    let r = repo().await;
    let club = mk(&r, "club", RightFlow::Down).await;
    let board = mk(&r, "board", RightFlow::Down).await;
    let interns = mk(&r, "interns", RightFlow::Down).await;
    common::link(&r, interns.id, board.id).await;
    r.add(interns.id, Member::Person(person("intern")), "")
        .await
        .unwrap();
    r.add(board.id, Member::Person(person("chair")), "")
        .await
        .unwrap();
    r.grant(club.id, Member::Group(board.id), Right::Manage)
        .await
        .unwrap();
    assert!(may(&r, &club, "chair").await);
    assert!(!may(&r, &club, "intern").await);
    // explicitly granted, the intern does hold it
    r.grant(club.id, Member::Person(person("intern")), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &club, "intern").await);
}

#[tokio::test]
async fn provisional_a_child_passing_up_cannot_reach_a_parent_that_says_none() {
    let r = repo().await;
    let victim = mk(&r, "victim", RightFlow::None).await;
    let child = mk(&r, "child", RightFlow::Up).await;
    common::link(&r, child.id, victim.id).await;
    r.grant(child.id, Member::Person(person("mallory")), Right::Manage)
        .await
        .unwrap();
    assert!(may(&r, &child, "mallory").await);
    assert!(!may(&r, &victim, "mallory").await);
    // a parent that passes down does not pull it either: both must pass the same way
    r.set_right_flow(victim.id, RightFlow::Down).await.unwrap();
    assert!(!may(&r, &victim, "mallory").await);
}

#[tokio::test]
async fn provisional_a_right_does_not_move_to_the_members_of_a_new_parent() {
    let r = repo().await;
    let target = mk(&r, "target", RightFlow::None).await;
    let team = mk(&r, "team", RightFlow::Up).await;
    let dept = mk(&r, "dept", RightFlow::Up).await;
    r.add(dept.id, Member::Person(person("head")), "")
        .await
        .unwrap();
    r.grant(target.id, Member::Group(team.id), Right::Manage)
        .await
        .unwrap();
    common::link(&r, team.id, dept.id).await;
    assert!(!may(&r, &target, "head").await);
}

#[tokio::test]
async fn provisional_members_of_an_up_child_under_an_up_holder_receive_its_rights() {
    let r = repo().await;
    let target = mk(&r, "target", RightFlow::None).await;
    let holder = mk(&r, "holder", RightFlow::Up).await;
    let child = mk(&r, "child", RightFlow::Up).await;
    let grandchild = mk(&r, "grandchild", RightFlow::Up).await;
    common::link(&r, child.id, holder.id).await;
    common::link(&r, grandchild.id, child.id).await;
    r.add(grandchild.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.grant(target.id, Member::Group(holder.id), Right::Read)
        .await
        .unwrap();
    assert!(
        may(&r, &target, "ada").await,
        "up along a chain of groups that all pass up"
    );
    r.set_right_flow(child.id, RightFlow::None).await.unwrap();
    assert!(!may(&r, &target, "ada").await, "the chain is broken");
    r.set_right_flow(child.id, RightFlow::Up).await.unwrap();
    r.set_right_flow(holder.id, RightFlow::Down).await.unwrap();
    assert!(
        !may(&r, &target, "ada").await,
        "the holder does not pass up"
    );
}

// ---- in doubt, no ----

#[tokio::test]
async fn an_unknown_group_is_an_error_not_a_yes() {
    let r = repo().await;
    let err = r
        .may_do(ObjectId::new(), &person("ada"), Right::Read)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NotFound { .. }));
    let g = mk(&r, "g", RightFlow::None).await;
    assert!(!r.may_do(g.id, " ", Right::Read).await.unwrap());
}

#[tokio::test]
async fn an_unknown_setting_in_the_store_reads_as_none() {
    let (r, store) = setup().await;
    let p = mk(&r, "p", RightFlow::Down).await;
    let c = mk(&r, "c", RightFlow::Down).await;
    common::link(&r, c.id, p.id).await;
    r.grant(p.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &c, "ada").await);
    let mut o = store.get(c.id).await.unwrap().unwrap();
    o.content["fs:rightFlow"] = "sideways".into();
    store.put(o, common::ME).await.unwrap();
    assert!(!may(&r, &c, "ada").await);
}

#[tokio::test]
async fn unreadable_objects_never_grant() {
    let (r, store) = setup().await;
    let g = mk(&r, "g", RightFlow::None).await;
    let d = r
        .grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    // the delegation loses its holder field: it cannot be read, so it counts for nobody
    let mut o = store.get(d.id).await.unwrap().unwrap();
    o.index.remove("holder");
    store.put(o, common::ME).await.unwrap();
    assert!(!may(&r, &g, "ada").await);
    // a right with a name this program does not know
    let d2 = r
        .grant(g.id, Member::Person(person("bo")), Right::Read)
        .await
        .unwrap();
    let mut o = store.get(d2.id).await.unwrap().unwrap();
    o.index.insert("right".into(), vec!["superuser".into()]);
    store.put(o, common::ME).await.unwrap();
    assert!(!may(&r, &g, "bo").await);
}

#[tokio::test]
async fn a_failing_read_is_an_error_never_a_yes() {
    let (r, store) = setup().await;
    let g = mk(&r, "g", RightFlow::None).await;
    r.grant(g.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    for failing in ["delegation", "membership", "groups"] {
        let broken = faulty_repo(
            &store,
            |_| false,
            move |q| match failing {
                "delegation" => q.object_type.as_deref() == Some("Delegation"),
                "membership" => q.object_type.as_deref() == Some("Relationship"),
                _ => q.object_type.as_deref() == Some("Group"),
            },
        );
        assert!(
            broken
                .may_do(g.id, &person("ada"), Right::Read)
                .await
                .is_err(),
            "{failing}"
        );
    }
}

#[tokio::test]
async fn a_loop_in_the_data_answers_no() {
    let (r, store) = setup().await;
    let a = mk(&r, "a", RightFlow::Down).await;
    let b = mk(&r, "b", RightFlow::Down).await;
    let c = mk(&r, "c", RightFlow::Down).await;
    common::link(&r, b.id, a.id).await;
    common::link(&r, c.id, b.id).await;
    r.grant(a.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    assert!(may(&r, &c, "ada").await);
    force_parent(&store, a.id, c.id).await;
    for g in [&a, &b, &c] {
        assert!(!may(&r, g, "ada").await, "{}", g.name);
    }
    assert_eq!(
        Graph::from_groups(&[]).cyclic_nodes().len(),
        0,
        "no groups, no loop"
    );
}

#[tokio::test]
async fn only_the_looped_part_is_refused() {
    let (r, store) = setup().await;
    let a = mk(&r, "a", RightFlow::Down).await;
    let b = mk(&r, "b", RightFlow::Down).await;
    let side = mk(&r, "side", RightFlow::Down).await;
    common::link(&r, b.id, a.id).await;
    force_parent(&store, a.id, b.id).await;
    r.grant(side.id, Member::Person(person("ada")), Right::Read)
        .await
        .unwrap();
    assert!(
        may(&r, &side, "ada").await,
        "a group apart from the loop is fine"
    );
}

// ---- the two clauses ----

/// Small deterministic generator, so the check below needs no extra crate.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        usize::try_from(self.0 >> 33).unwrap() % n
    }
}

fn group(id: ObjectId, flow: RightFlow, parents: Vec<ObjectId>) -> Group {
    Group {
        id,
        iri: String::new(),
        name: "g".into(),
        kind: String::new(),
        parents,
        right_flow: flow,
    }
}

#[test]
fn sources_is_the_inverse_of_reach() {
    let mut rng = Lcg(7);
    let flows = [RightFlow::None, RightFlow::Down, RightFlow::Up];
    for _ in 0..200 {
        let n = 2 + rng.next(7);
        let ids: Vec<_> = (0..n).map(|_| ObjectId::new()).collect();
        let groups: Vec<_> = (0..n)
            .map(|i| {
                // parents only from lower indexes: no loops
                let parents = (0..i)
                    .filter(|_| rng.next(3) == 0)
                    .map(|j| ids[j])
                    .collect();
                group(ids[i], flows[rng.next(3)], parents)
            })
            .collect();
        let graph = Graph::from_groups(&groups);
        let settings: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g.right_flow)).collect();
        for target in &ids {
            let brute: std::collections::BTreeSet<_> = ids
                .iter()
                .filter(|s| reach(&graph, &settings, **s).contains(target))
                .copied()
                .collect();
            assert_eq!(sources(&graph, &settings, *target), brute);
        }
    }
}

#[test]
fn reach_and_sources_of_an_unknown_group_are_empty() {
    let g = Graph::default();
    let s = BTreeMap::new();
    let id = ObjectId::new();
    assert!(reach(&g, &s, id).is_empty());
    assert!(sources(&g, &s, id).is_empty());
    assert!(fs_groups_core::right_flow::recipients(&g, &s, id).is_empty());
}

#[test]
fn a_nested_edge_is_expressible_exactly_when_the_child_is_a_recipient_of_the_parent() {
    let mut rng = Lcg(11);
    let flows = [RightFlow::None, RightFlow::Down, RightFlow::Up];
    for _ in 0..200 {
        let n = 2 + rng.next(7);
        let ids: Vec<_> = (0..n).map(|_| ObjectId::new()).collect();
        let groups: Vec<_> = (0..n)
            .map(|i| {
                let parents = (0..i)
                    .filter(|_| rng.next(3) == 0)
                    .map(|j| ids[j])
                    .collect();
                group(ids[i], flows[rng.next(3)], parents)
            })
            .collect();
        let graph = Graph::from_groups(&groups);
        let settings: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g.right_flow)).collect();
        for g in &groups {
            for p in &g.parents {
                let receives =
                    fs_groups_core::right_flow::recipients(&graph, &settings, *p).contains(&g.id);
                assert_eq!(
                    fs_groups_core::right_flow::directory_expresses(&settings, g.id, *p),
                    receives
                );
            }
        }
    }
}
