//! The safe reading of up and down (provisional) and "in doubt, no" for `may_do`.

mod common;

use common::{faulty_repo, force_parent, person, repo, setup};
use fs_groups_core::{Error, Graph, Group, GroupRepo, Member, Right, RightFlow};
use fs_objects::ObjectId;

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
