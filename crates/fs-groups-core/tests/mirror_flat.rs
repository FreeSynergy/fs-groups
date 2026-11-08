//! Mirror option B (ADR 0056): a nested edge goes to the directory only where the rights really pass
//! on (provisional: both the child and the parent pass `Up`, which is what Kanidm does: members of a
//! child get what the parent holds). Everywhere else the directory gets a flat list: the parent lists
//! the people of that child itself. Nobody who belongs loses anything `effective` says.

mod common;

use std::collections::BTreeSet;

use common::{FakeDirectory, link, person, repo};
use fs_groups_core::{Divergence, Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_objects::ObjectId;

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn put(r: &GroupRepo, g: &Group, who: &str) {
    r.add(g.id, Member::Person(person(who)), "").await.unwrap();
}

/// Everybody the directory counts as a member of `id`: listed, or through nesting.
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

fn listed(dir: &FakeDirectory, id: ObjectId) -> BTreeSet<String> {
    dir.get(id).unwrap().members.into_iter().collect()
}

/// Nobody who belongs is lost, and nobody who does not belong is added: the directory's closure of a
/// group is what `effective` says.
async fn check_all(r: &GroupRepo, dir: &FakeDirectory, groups: &[&Group]) {
    for g in groups {
        let effective: BTreeSet<_> = r
            .effective(g.id)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.person)
            .collect();
        assert_eq!(
            closure(dir, g.id),
            effective,
            "{} loses or gains someone",
            g.name
        );
    }
}

#[tokio::test]
async fn an_edge_is_nested_only_where_both_ends_pass_up() {
    let r = repo().await;
    let parent = mk(&r, "parent", RightFlow::Up).await;
    let up = mk(&r, "up", RightFlow::Up).await;
    let none = mk(&r, "none", RightFlow::None).await;
    for (g, who) in [(&parent, "p"), (&up, "u"), (&none, "n")] {
        put(&r, g, who).await;
    }
    link(&r, up.id, parent.id).await;
    link(&r, none.id, parent.id).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(parent.id).unwrap().nested, [up.id]);
    assert_eq!(listed(&dir, parent.id), [person("p"), person("n")].into());
    check_all(&r, &dir, &[&parent, &up, &none]).await;
}

#[tokio::test]
async fn a_parent_that_does_not_pass_up_gets_a_flat_list() {
    let r = repo().await;
    let parent = mk(&r, "parent", RightFlow::Down).await;
    let child = mk(&r, "child", RightFlow::Up).await;
    put(&r, &child, "c").await;
    link(&r, child.id, parent.id).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(parent.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
    assert_eq!(listed(&dir, parent.id), [person("c")].into());
    check_all(&r, &dir, &[&parent, &child]).await;
}

#[tokio::test]
async fn chains_nest_where_they_pass_and_flatten_below_a_break() {
    let repo = repo().await;
    // a <- b <- c <- d, with c not passing on: b is nested in a, c and d are listed in b
    let a = mk(&repo, "a", RightFlow::Up).await;
    let b = mk(&repo, "b", RightFlow::Up).await;
    let c = mk(&repo, "c", RightFlow::None).await;
    let d = mk(&repo, "d", RightFlow::Up).await;
    for (g, who) in [(&a, "a"), (&b, "b"), (&c, "c"), (&d, "d")] {
        put(&repo, g, who).await;
    }
    link(&repo, b.id, a.id).await;
    link(&repo, c.id, b.id).await;
    link(&repo, d.id, c.id).await;
    let dir = FakeDirectory::default();
    repo.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(a.id).unwrap().nested, [b.id]);
    assert_eq!(
        dir.get(b.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
    assert_eq!(
        listed(&dir, b.id),
        [person("b"), person("c"), person("d")].into(),
        "the whole subtree of c, also what is below it"
    );
    assert_eq!(listed(&dir, a.id), [person("a")].into());
    check_all(&repo, &dir, &[&a, &b, &c, &d]).await;
}

#[tokio::test]
async fn a_diamond_is_nested_on_one_side_and_flat_on_the_other() {
    let r = repo().await;
    let top = mk(&r, "top", RightFlow::Up).await;
    let left = mk(&r, "left", RightFlow::Up).await;
    let right = mk(&r, "right", RightFlow::None).await;
    let bottom = mk(&r, "bottom", RightFlow::Up).await;
    for (g, who) in [(&top, "t"), (&left, "l"), (&right, "r"), (&bottom, "b")] {
        put(&r, g, who).await;
    }
    for (c, p) in [
        (&left, &top),
        (&right, &top),
        (&bottom, &left),
        (&bottom, &right),
    ] {
        link(&r, c.id, p.id).await;
    }
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(left.id).unwrap().nested, [bottom.id]);
    assert_eq!(
        dir.get(right.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
    assert!(listed(&dir, right.id).contains(&person("b")), "listed flat");
    assert_eq!(dir.get(top.id).unwrap().nested, [left.id]);
    check_all(&r, &dir, &[&top, &left, &right, &bottom]).await;
}

#[tokio::test]
async fn a_change_of_setting_heals_the_directory_both_ways() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::Up).await,
        mk(&r, "c", RightFlow::Up).await,
    );
    put(&r, &p, "p").await;
    put(&r, &c, "c").await;
    link(&r, c.id, p.id).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(p.id).unwrap().nested, [c.id]);
    r.set_right_flow(c.id, RightFlow::None).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(report.updated, [p.id], "only the parent changes");
    assert_eq!(
        dir.get(p.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
    assert_eq!(listed(&dir, p.id), [person("p"), person("c")].into());
    check_all(&r, &dir, &[&p, &c]).await;
    r.set_right_flow(c.id, RightFlow::Up).await.unwrap();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(p.id).unwrap().nested, [c.id]);
    assert_eq!(listed(&dir, p.id), [person("p")].into());
    check_all(&r, &dir, &[&p, &c]).await;
}

#[tokio::test]
async fn the_old_nested_form_is_healed() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    put(&r, &c, "c").await;
    link(&r, c.id, p.id).await;
    // what an earlier version wrote: always nested, never flat
    let dir = FakeDirectory::default();
    let old = |g: &Group, members: &[&str], nested: &[ObjectId]| fs_groups_core::DirectoryGroup {
        id: g.id,
        name: g.name.clone(),
        members: members.iter().map(|m| person(m)).collect(),
        nested: nested.to_vec(),
    };
    dir.groups
        .lock()
        .unwrap()
        .insert(c.id, old(&c, &["c"], &[]));
    dir.groups
        .lock()
        .unwrap()
        .insert(p.id, old(&p, &[], &[c.id]));
    r.sync_to(&dir).await.unwrap();
    assert_eq!(
        dir.get(p.id).unwrap().nested,
        [] as [fs_objects::ObjectId; 0]
    );
    assert_eq!(listed(&dir, p.id), [person("c")].into());
}

#[tokio::test]
async fn an_edge_that_is_removed_leaves_the_flat_list() {
    let r = repo().await;
    let (p, c) = (
        mk(&r, "p", RightFlow::None).await,
        mk(&r, "c", RightFlow::None).await,
    );
    put(&r, &c, "c").await;
    link(&r, c.id, p.id).await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(listed(&dir, p.id), [person("c")].into());
    r.remove_parent(c.id, p.id).await.unwrap();
    r.sync_to(&dir).await.unwrap();
    assert!(listed(&dir, p.id).is_empty());
    assert!(r.sync_to(&dir).await.unwrap().is_quiet());
}

#[tokio::test]
async fn a_person_in_two_children_is_listed_once_and_a_second_run_writes_nothing() {
    let r = repo().await;
    let p = mk(&r, "p", RightFlow::None).await;
    let (c1, c2) = (
        mk(&r, "c1", RightFlow::None).await,
        mk(&r, "c2", RightFlow::None).await,
    );
    for c in [&c1, &c2] {
        put(&r, c, "both").await;
        link(&r, c.id, p.id).await;
    }
    put(&r, &p, "both").await;
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(p.id).unwrap().members, [person("both")]);
    let writes = dir.writes();
    r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.writes(), writes);
}

#[tokio::test]
async fn every_edge_is_a_divergence_the_nested_ones_too() {
    let r = repo().await;
    let p = mk(&r, "p", RightFlow::Up).await;
    let (nested, flat) = (
        mk(&r, "nested", RightFlow::Up).await,
        mk(&r, "flat", RightFlow::None).await,
    );
    put(&r, &nested, "n").await;
    link(&r, nested.id, p.id).await;
    link(&r, flat.id, p.id).await;
    let dir = FakeDirectory::default();
    let report = r.sync_to(&dir).await.unwrap();
    assert_eq!(dir.get(p.id).unwrap().nested, [nested.id]);
    let mut want = vec![
        Divergence {
            group: nested.id,
            parent: p.id,
        },
        Divergence {
            group: flat.id,
            parent: p.id,
        },
    ];
    want.sort_by_key(|d| (d.group, d.parent));
    assert_eq!(report.divergences, want);
    // the directory makes n a member of p; may_do does not (direct members only)
    assert!(closure(&dir, p.id).contains(&person("n")));
    let target = r.create("target", "t").await.unwrap();
    r.grant(target.id, Member::Group(p.id), Right::Read)
        .await
        .unwrap();
    assert!(
        !r.may_do(target.id, &person("n"), Right::Read)
            .await
            .unwrap()
    );
}
