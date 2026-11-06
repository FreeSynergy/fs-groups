//! The sync report names nested edges where the directory gives more than the groups' rights do
//! (provisional: with the safe reading that is every nested edge).

mod common;

use common::{FakeDirectory, repo};
use fs_groups_core::{Divergence, Group, GroupRepo, Member, Mirror, Right, RightFlow};

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

#[tokio::test]
async fn provisional_every_nested_edge_is_reported_and_still_applied() {
    let r = repo().await;
    let dept = mk(&r, "dept", RightFlow::Down).await;
    let mut want = vec![];
    for flow in [RightFlow::Up, RightFlow::None, RightFlow::Down] {
        let child = mk(&r, "child", flow).await;
        common::link(&r, child.id, dept.id).await;
        want.push(Divergence {
            group: child.id,
            parent: dept.id,
        });
    }
    want.sort_by_key(|d| (d.group, d.parent));
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.divergences, want, "whatever the children set");
    // what is applied did not adapt
    assert_eq!(dir.get(dept.id).unwrap().nested.len(), 3);
    // reported again on every run, since the cause is still there; nothing more is written
    let writes = dir.writes();
    let second = r.sync_to(&dir).await.unwrap();
    assert_eq!(second.divergences, want);
    assert_eq!(dir.writes(), writes);
}

#[tokio::test]
async fn the_reported_edge_is_one_where_may_do_and_the_directory_differ() {
    let r = repo().await;
    let parent = mk(&r, "parent", RightFlow::Down).await;
    let child = mk(&r, "child", RightFlow::Up).await;
    common::link(&r, child.id, parent.id).await;
    r.add(child.id, Member::Person(common::person("ada")), "")
        .await
        .unwrap();
    r.grant(parent.id, Member::Group(parent.id), Right::Read)
        .await
        .unwrap();
    // ada is in the child; the directory makes her memberof the parent and so gives her its right
    assert!(
        !r.may_do(parent.id, &common::person("ada"), Right::Read)
            .await
            .unwrap()
    );
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(
        report.divergences,
        vec![Divergence {
            group: child.id,
            parent: parent.id
        }]
    );
}

#[tokio::test]
async fn an_edge_that_is_only_requested_is_not_mirrored() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    r.request_parent(c.id, p.id).await.unwrap();
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    assert!(report.divergences.is_empty());
    assert!(dir.get(p.id).unwrap().nested.is_empty());
}
