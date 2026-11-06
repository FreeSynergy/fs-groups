//! The sync report names nested edges a directory cannot hold as the groups' right-flow settings say.

mod common;

use common::{FakeDirectory, repo};
use fs_groups_core::{Divergence, Group, GroupRepo, Mirror, RightFlow};

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

#[tokio::test]
async fn the_mirror_reports_nested_edges_kanidm_cannot_express_and_still_applies_them() {
    let r = repo().await;
    let dept = mk(&r, "dept", RightFlow::None).await;
    let up = mk(&r, "up", RightFlow::Up).await;
    let none = mk(&r, "none", RightFlow::None).await;
    let down = mk(&r, "down", RightFlow::Down).await;
    for c in [&up, &none, &down] {
        r.add_parent(c.id, dept.id).await.unwrap();
    }
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    let mut want = vec![
        Divergence {
            group: none.id,
            parent: dept.id,
        },
        Divergence {
            group: down.id,
            parent: dept.id,
        },
    ];
    want.sort_by_key(|d| (d.group, d.parent));
    assert_eq!(
        report.divergences, want,
        "the up child is expressible, a group without a parent has no edge"
    );
    // what is applied did not adapt
    assert_eq!(dir.get(dept.id).unwrap().nested.len(), 3);
    // reported again on every run, since the cause is still there; nothing more is written
    let writes = dir.writes();
    let second = r.sync_to(&dir).await.unwrap();
    assert_eq!(second.divergences, want);
    assert_eq!(dir.writes(), writes);
}
