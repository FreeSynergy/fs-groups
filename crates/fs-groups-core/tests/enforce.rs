//! `enforce`, the one place that knows "an agent is never admin", on hand-made plans.

use std::collections::{BTreeMap, BTreeSet};

use fs_groups_core::{AdminRule, DirectoryGroup, Verdict, enforce};
use fs_objects::ObjectId;

fn group(id: ObjectId, members: &[&str], nested: &[ObjectId]) -> DirectoryGroup {
    DirectoryGroup {
        id,
        name: "g".into(),
        members: members.iter().map(|m| (*m).to_string()).collect(),
        nested: nested.to_vec(),
    }
}

fn verdicts() -> BTreeMap<String, Verdict> {
    [
        ("ada", Verdict::Human),
        ("bot", Verdict::Agent),
        ("who", Verdict::Unknown),
    ]
    .into_iter()
    .map(|(p, v)| (p.to_string(), v))
    .collect()
}

fn strict(ids: &[ObjectId]) -> AdminRule {
    AdminRule {
        strict: ids.iter().copied().collect::<BTreeSet<_>>(),
        lenient_all: false,
    }
}

#[test]
fn a_direct_listing_is_dropped_and_named() {
    let h = ObjectId::new();
    let mut plan = BTreeMap::from([(h, group(h, &["ada", "bot", "who"], &[]))]);
    let done = enforce(&mut plan, &strict(&[h]), &verdicts());
    assert_eq!(plan[&h].members, ["ada"]);
    assert_eq!(
        done.left_out,
        [(h, vec!["bot".to_string(), "who".to_string()])]
    );
}

#[test]
fn a_nested_edge_whose_child_holds_one_becomes_a_listing_without_them() {
    let (h, c, d) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
    let mut plan = BTreeMap::from([
        (h, group(h, &[], &[c])),
        (c, group(c, &["ada"], &[d])),
        (d, group(d, &["bot"], &[])),
    ]);
    enforce(&mut plan, &strict(&[h]), &verdicts());
    assert_eq!(plan[&h].nested, [] as [fs_objects::ObjectId; 0]);
    assert_eq!(plan[&h].members, ["ada"]);
    // the child and the grandchild are not administrative and keep their lists and their edge
    assert_eq!(plan[&c].nested, [d]);
    assert_eq!(plan[&d].members, ["bot"]);
}

#[test]
fn an_agent_in_a_group_that_is_not_administrative_stays() {
    let (h, c) = (ObjectId::new(), ObjectId::new());
    let mut plan = BTreeMap::from([(h, group(h, &[], &[])), (c, group(c, &["bot"], &[]))]);
    let done = enforce(&mut plan, &strict(&[h]), &verdicts());
    assert_eq!(plan[&c].members, ["bot"]);
    assert_eq!(
        done.left_out,
        [] as [(fs_objects::ObjectId, std::vec::Vec<std::string::String>); 0]
    );
}

#[test]
fn a_person_missing_from_the_verdicts_counts_as_unknown() {
    let h = ObjectId::new();
    let mut plan = BTreeMap::from([(h, group(h, &["ada", "stranger"], &[]))]);
    enforce(&mut plan, &strict(&[h]), &verdicts());
    assert_eq!(plan[&h].members, ["ada"]);
}

#[test]
fn where_any_group_may_be_administrative_only_known_agents_go_and_unknowns_are_reported() {
    let (a, b) = (ObjectId::new(), ObjectId::new());
    let mut plan = BTreeMap::from([
        (a, group(a, &["ada", "bot", "who"], &[])),
        (b, group(b, &["who"], &[])),
    ]);
    let rule = AdminRule {
        strict: BTreeSet::new(),
        lenient_all: true,
    };
    let done = enforce(&mut plan, &rule, &verdicts());
    assert_eq!(plan[&a].members, ["ada", "who"]);
    assert_eq!(plan[&b].members, ["who"]);
    assert_eq!(done.left_out, [(a, vec!["bot".to_string()])]);
    assert_eq!(done.unchecked.len(), 2);
}

#[test]
fn a_chain_of_administrative_groups_is_made_clean_all_the_way_up() {
    let (top, mid, low) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
    let mut plan = BTreeMap::from([
        (top, group(top, &[], &[mid])),
        (mid, group(mid, &[], &[low])),
        (low, group(low, &["ada", "bot"], &[])),
    ]);
    let done = enforce(&mut plan, &strict(&[top, mid]), &verdicts());
    for g in [top, mid] {
        let seen = fs_groups_core::enforce_closure(&plan, g);
        assert!(!seen.contains("bot"), "{seen:?}");
        assert!(seen.contains("ada"));
    }
    assert_eq!(
        done.left_out.len(),
        2,
        "named in both administrative groups"
    );
}

#[test]
fn a_loop_in_the_plan_ends() {
    let (a, b) = (ObjectId::new(), ObjectId::new());
    let mut plan = BTreeMap::from([(a, group(a, &["bot"], &[b])), (b, group(b, &["ada"], &[a]))]);
    enforce(&mut plan, &strict(&[a, b]), &verdicts());
    for g in [a, b] {
        assert!(!fs_groups_core::enforce_closure(&plan, g).contains("bot"));
    }
}
