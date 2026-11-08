//! Nesting: several parents, no cycles, ancestors and descendants once each.

mod common;

use common::{force_parent, repo, setup};
use fs_groups_core::{Error, Group, GroupRepo};

async fn make(r: &GroupRepo, names: &[&str]) -> Vec<Group> {
    let mut v = vec![];
    for n in names {
        v.push(r.create(n, "team").await.unwrap());
    }
    v
}

#[tokio::test]
async fn a_group_may_have_several_parents() {
    let r = repo().await;
    let g = make(&r, &["working group", "dept a", "dept b"]).await;
    common::link(&r, g[0].id, g[1].id).await;
    let wg = common::link(&r, g[0].id, g[2].id).await;
    assert_eq!(wg.parents, [g[1].id, g[2].id]);
    assert_eq!(r.parents(g[0].id).await.unwrap(), [g[1].id, g[2].id]);
    assert_eq!(r.children(g[1].id).await.unwrap(), [g[0].id]);
}

#[tokio::test]
async fn asking_for_a_link_that_exists_changes_nothing() {
    let r = repo().await;
    let g = make(&r, &["a", "b"]).await;
    common::link(&r, g[0].id, g[1].id).await;
    assert!(matches!(
        r.request_parent(g[0].id, g[1].id).await,
        Err(Error::Conflict(_))
    ));
    assert_eq!(r.parents(g[0].id).await.unwrap(), [g[1].id]);
}

#[tokio::test]
async fn a_group_cannot_be_its_own_parent() {
    let r = repo().await;
    let g = make(&r, &["a"]).await;
    let Err(Error::Cycle { path, .. }) = r.request_parent(g[0].id, g[0].id).await else {
        panic!("expected a cycle error")
    };
    assert_eq!(path, [g[0].id]);
    assert_eq!(
        r.parents(g[0].id).await.unwrap(),
        [] as [fs_objects::ObjectId; 0]
    );
}

#[tokio::test]
async fn a_loop_is_refused_with_the_chain_that_closes_it() {
    let r = repo().await;
    let g = make(&r, &["a", "b", "c"]).await;
    common::link(&r, g[1].id, g[0].id).await; // a contains b
    common::link(&r, g[2].id, g[1].id).await; // b contains c
    let err = r.request_parent(g[0].id, g[2].id).await.unwrap_err(); // c contains a: loop
    let Error::Cycle {
        group,
        parent,
        path,
    } = &err
    else {
        panic!("expected a cycle error, got {err}")
    };
    assert_eq!((*group, *parent), (g[0].id, g[2].id));
    assert_eq!(path, &[g[2].id, g[1].id, g[0].id]);
    assert!(err.to_string().contains("ancestor"));
    assert!(
        r.parents(g[0].id).await.unwrap().is_empty(),
        "nothing was written"
    );
}

#[tokio::test]
async fn a_long_chain_is_walked_and_its_loop_refused() {
    let r = repo().await;
    let mut ids = vec![r.create("g0", "team").await.unwrap().id];
    for i in 1..150 {
        let g = r.create(&format!("g{i}"), "team").await.unwrap();
        common::link(&r, g.id, *ids.last().unwrap()).await;
        ids.push(g.id);
    }
    assert_eq!(r.ancestors(ids[149]).await.unwrap().len(), 149);
    assert_eq!(r.descendants(ids[0]).await.unwrap().len(), 149);
    assert!(matches!(
        r.request_parent(ids[0], ids[149]).await,
        Err(Error::Cycle { .. })
    ));
}

#[tokio::test]
async fn a_diamond_is_fine_and_each_group_is_visited_once() {
    let r = repo().await;
    let g = make(&r, &["top", "left", "right", "bottom"]).await;
    for (child, parent) in [(1, 0), (2, 0), (3, 1), (3, 2)] {
        common::link(&r, g[child].id, g[parent].id).await;
    }
    let up = r.ancestors(g[3].id).await.unwrap();
    assert_eq!(up.len(), 3);
    assert_eq!(up[2], g[0].id, "nearest first");
    assert_eq!(r.descendants(g[0].id).await.unwrap().len(), 3);
    // closing the diamond upward is a loop
    assert!(matches!(
        r.request_parent(g[0].id, g[3].id).await,
        Err(Error::Cycle { .. })
    ));
}

#[tokio::test]
async fn walks_end_on_bad_data_that_has_a_cycle() {
    let (r, store) = setup().await;
    let g = make(&r, &["a", "b", "c"]).await;
    common::link(&r, g[1].id, g[0].id).await;
    common::link(&r, g[2].id, g[1].id).await;
    force_parent(&store, g[0].id, g[2].id).await; // a loop planted from outside
    assert_eq!(r.ancestors(g[2].id).await.unwrap().len(), 2);
    assert_eq!(r.descendants(g[0].id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn removing_a_link_leaves_both_groups() {
    let r = repo().await;
    let g = make(&r, &["a", "b"]).await;
    common::link(&r, g[0].id, g[1].id).await;
    let a = r.remove_parent(g[0].id, g[1].id).await.unwrap();
    assert_eq!(a.parents, [] as [fs_objects::ObjectId; 0]);
    assert!(r.get(g[1].id).await.unwrap().is_some());
    // now the other direction is allowed
    common::link(&r, g[1].id, g[0].id).await;
}
