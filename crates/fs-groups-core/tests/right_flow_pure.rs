//! The two clauses of `right_flow` on random small graphs, without a store.

use std::collections::BTreeMap;

use fs_groups_core::right_flow::{reach, sources};
use fs_groups_core::{Graph, Group, RightFlow};
use fs_objects::ObjectId;

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
        envelope: fs_objects::Envelope::new("Group", "x"),
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
    assert_eq!(reach(&g, &s, id), [] as [fs_objects::ObjectId; 0]);
    assert!(sources(&g, &s, id).is_empty());
    assert_eq!(
        fs_groups_core::right_flow::recipients(&s, id),
        [] as [fs_objects::ObjectId; 0]
    );
}

#[test]
fn an_edge_matches_may_do_exactly_when_the_child_is_a_recipient_and_nests_when_both_pass_up() {
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
        let settings: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g.right_flow)).collect();
        for g in &groups {
            for p in &g.parents {
                let receives =
                    fs_groups_core::right_flow::recipients(&settings, *p).contains(&g.id);
                assert_eq!(
                    fs_groups_core::right_flow::directory_expresses(&settings, g.id, *p),
                    receives
                );
                assert!(
                    !receives,
                    "direct members only: no child is ever a recipient"
                );
                assert_eq!(
                    fs_groups_core::right_flow::directory_nests(&settings, g.id, *p),
                    g.right_flow == RightFlow::Up && settings[p] == RightFlow::Up
                );
            }
        }
    }
}
