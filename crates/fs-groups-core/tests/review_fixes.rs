//! Findings of the independent review, each pinned by a test that failed before the fix.
//! The races are forced with a backend that answers reads late (`common::Slow`).

mod common;

use common::{at, person, repo, setup, slow_repos};
use fs_groups_core::{Divergence, GroupRepo, InvitationState, Member, Mirror, Right, RightFlow};

// ---- A: accept ----

#[tokio::test]
async fn only_the_invitee_can_accept() {
    let r = repo().await;
    let g = r.create("Club", "club").await.unwrap();
    let inv = r.invite(g.id, &person("ada"), "", None).await.unwrap();
    assert!(r.accept(inv.id, &person("mallory"), at(1)).await.is_err());
    assert!(!r.is_member(g.id, &person("mallory")).await.unwrap());
    assert_eq!(
        r.invitations(g.id).await.unwrap()[0].state,
        InvitationState::Pending,
        "a wrong person does not use the invitation up"
    );
    r.accept(inv.id, &person("ada"), at(1)).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_double_accept_makes_one_membership() {
    for _ in 0..5 {
        let rs = slow_repos(2).await;
        let g = rs[0].create("Club", "club").await.unwrap();
        let inv = rs[0].invite(g.id, &person("ada"), "", None).await.unwrap();
        let ada = person("ada");
        let (a, b) = tokio::join!(
            rs[0].accept(inv.id, &ada, at(1)),
            rs[1].accept(inv.id, &ada, at(1)),
        );
        assert!(a.is_ok() != b.is_ok(), "exactly one accept wins");
        assert_eq!(rs[0].direct(g.id).await.unwrap().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_revoke_that_succeeded_is_never_followed_by_accepted() {
    for _ in 0..5 {
        let rs = slow_repos(2).await;
        let g = rs[0].create("Club", "club").await.unwrap();
        let inv = rs[0].invite(g.id, &person("ada"), "", None).await.unwrap();
        let ada = person("ada");
        let (acc, rev) = tokio::join!(rs[0].accept(inv.id, &ada, at(1)), rs[1].revoke(inv.id),);
        let state = rs[0].invitations(g.id).await.unwrap()[0].state;
        if rev.is_ok() {
            assert!(acc.is_err());
            assert_eq!(state, InvitationState::Revoked);
            assert!(rs[0].direct(g.id).await.unwrap().is_empty());
        } else {
            assert_eq!(state, InvitationState::Accepted);
        }
    }
}

// ---- B: writes that read first ----

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_updates_of_one_group_all_stay() {
    for _ in 0..5 {
        let rs = slow_repos(3).await;
        let g = rs[0].create("old", "old").await.unwrap();
        let p = rs[0].create("parent", "t").await.unwrap();
        let ask = rs[0].request_parent(g.id, p.id).await.unwrap();
        let (a, b, c, d) = tokio::join!(
            rs[0].rename(g.id, "new"),
            rs[1].set_kind(g.id, "new-kind"),
            rs[2].set_right_flow(g.id, RightFlow::Down),
            rs[0].approve_parent(ask.id),
        );
        for done in [a.map(|_| ()), b.map(|_| ()), c.map(|_| ()), d.map(|_| ())] {
            done.unwrap();
        }
        let now = rs[0].get(g.id).await.unwrap().unwrap();
        assert_eq!(
            (now.name.as_str(), now.kind.as_str(), now.right_flow),
            ("new", "new-kind", RightFlow::Down)
        );
        assert_eq!(now.parents, vec![p.id], "the parent link is not lost");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_update_does_not_bring_a_deleted_group_back() {
    for offset in 1..16u64 {
        let rs = slow_repos(2).await;
        let g = rs[0].create("g", "t").await.unwrap();
        // Sweep the start of the rename: for some offset it reads the group while the delete is
        // still running and writes it back after the delete is done.
        let late_rename = async {
            tokio::time::sleep(std::time::Duration::from_millis(offset)).await;
            rs[0].rename(g.id, "again").await
        };
        let (_, _) = tokio::join!(late_rename, rs[1].delete(g.id));
        assert!(rs[0].get(g.id).await.unwrap().is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_same_grant_or_add_twice_is_one() {
    for _ in 0..5 {
        let rs = slow_repos(2).await;
        let g = rs[0].create("g", "t").await.unwrap();
        let who = Member::Person(person("ada"));
        let (a, b) = tokio::join!(
            rs[0].grant(g.id, who.clone(), Right::Read),
            rs[1].grant(g.id, who.clone(), Right::Read),
        );
        assert_eq!(a.unwrap().id, b.unwrap().id);
        assert_eq!(rs[0].delegations(g.id).await.unwrap().len(), 1);
        let (a, b) = tokio::join!(rs[0].add(g.id, who.clone(), "r"), rs[1].add(g.id, who, "r"));
        assert_eq!(a.unwrap().id, b.unwrap().id);
        assert_eq!(rs[0].direct(g.id).await.unwrap().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_right_stays_for_a_deleted_holder_group() {
    for _ in 0..5 {
        let rs = slow_repos(2).await;
        let g = rs[0].create("g", "t").await.unwrap();
        let h = rs[0].create("h", "t").await.unwrap();
        let (_, _) = tokio::join!(
            rs[0].grant(g.id, Member::Group(h.id), Right::Read),
            rs[1].delete(h.id),
        );
        let left = rs[0].delegations(g.id).await.unwrap();
        assert!(left.is_empty(), "a delegation to a deleted group: {left:?}");
    }
}

#[tokio::test]
async fn may_do_ignores_a_holder_group_that_is_gone() {
    let (r, store) = setup().await;
    let g = r.create("g", "t").await.unwrap();
    let h = r.create("h", "t").await.unwrap();
    r.add(h.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.grant(g.id, Member::Group(h.id), Right::Read)
        .await
        .unwrap();
    store.delete(h.id, common::ME).await.unwrap(); // bypasses the cascade
    assert!(!r.may_do(g.id, &person("ada"), Right::Read).await.unwrap());
}

#[test]
fn reach_of_an_unknown_group_is_empty() {
    let none = std::collections::BTreeMap::new();
    let graph = fs_groups_core::Graph::default();
    let id = fs_store::ObjectId::new();
    assert!(fs_groups_core::right_flow::reach(&graph, &none, id).is_empty());
}

// ---- should fix: two instances over one store ----

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_instances_over_one_store_cannot_store_a_cycle() {
    for _ in 0..10 {
        let rs: Vec<GroupRepo> = slow_repos(2).await;
        let x = rs[0].create("x", "t").await.unwrap();
        let y = rs[0].create("y", "t").await.unwrap();
        // both requests are fine one at a time; approving both would close the loop
        let xy = rs[0].request_parent(x.id, y.id).await.unwrap();
        let yx = rs[1].request_parent(y.id, x.id).await.unwrap();
        let (a, b) = tokio::join!(rs[0].approve_parent(xy.id), rs[1].approve_parent(yx.id));
        assert!(a.is_ok() != b.is_ok(), "one approval, one refusal");
        let (gx, gy) = (
            rs[0].get(x.id).await.unwrap().unwrap(),
            rs[0].get(y.id).await.unwrap().unwrap(),
        );
        assert!(
            !(gx.parents.contains(&y.id) && gy.parents.contains(&x.id)),
            "a cycle was stored"
        );
    }
}

// ---- C: the divergence list ----

#[tokio::test]
async fn every_nested_edge_is_reported_since_the_directory_gives_more() {
    let r = repo().await;
    let parent = r.create("parent", "t").await.unwrap();
    let child = r.create("child", "t").await.unwrap();
    r.set_right_flow(child.id, RightFlow::Up).await.unwrap();
    common::link(&r, child.id, parent.id).await;
    r.add(child.id, Member::Person(person("ada")), "")
        .await
        .unwrap();
    r.grant(parent.id, Member::Group(parent.id), Right::Read)
        .await
        .unwrap();
    assert!(
        !r.may_do(parent.id, &person("ada"), Right::Read)
            .await
            .unwrap()
    );
    // the directory makes ada memberof the parent and so gives her its right
    assert_eq!(
        r.divergences().await.unwrap(),
        vec![Divergence {
            group: child.id,
            parent: parent.id
        }]
    );
}
