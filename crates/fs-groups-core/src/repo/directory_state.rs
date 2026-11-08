//! The wanted state of a directory, worked out from the groups in one pass.
//!
//! Pattern: Specification over the in-memory graph — option B (ADR 0056, provisional): a child is
//! nested only where `right_flow::directory_nests` says so, every other child is listed flat in the
//! parent. Nothing here knows the rule about agents; it only finds out **which groups are
//! administrative and what cannot be read**, with every unknown counted against the group (fail
//! closed). `mirror::enforce` applies the rule to the final plan.
//!
//! Everything in the context is read once and sorted by type, so an object whose type was changed, a
//! membership whose index and content disagree, or a group with a corrupt parent entry is noticed
//! and reported instead of silently missing.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::{ObjectId, Order, Query, StoredObject};

use super::codec;
use super::consts::{
    CONTEXT, DELEGATION_TYPE, GROUP_TYPE, INVITATION_TYPE, LEDGER_TYPE, MEMBERSHIP_TYPE,
    PARENT_REQUEST_TYPE,
};
use super::handle::GroupRepo;
use crate::error::Result;
use crate::graph::Graph;
use crate::mirror::{AdminRule, Scope, Wanted};
use crate::model::{Group, Member, Right, RightFlow};
use crate::right_flow;
use crate::traits::DirectoryGroup;

type People = BTreeMap<ObjectId, BTreeSet<(String, String)>>;

/// What cannot be read, and which groups it touches.
#[derive(Default)]
struct Doubt {
    /// Stored objects that cannot be read.
    unreadable: Vec<ObjectId>,
    /// Groups whose wanted list may be short (the group, and the groups above it), or all.
    uncertain: Scope,
    /// Groups that count as administrative because of an object that cannot be read, or all.
    admin: Scope,
}

impl Doubt {
    /// The groups above `group` (and it) list its people: they may be short.
    fn above(&mut self, graph: &Graph, group: ObjectId) {
        self.uncertain.groups.insert(group);
        self.uncertain.groups.extend(graph.ancestors(group));
    }

    /// An object that cannot be read touches `group` if its index still says which; then the group
    /// counts as administrative. If not, it could not be placed at all.
    fn touches(&mut self, graph: &Graph, id: ObjectId, group: Option<ObjectId>) {
        self.unreadable.push(id);
        if let Some(group) = group {
            self.above(graph, group);
            self.admin.groups.insert(group);
        } else {
            self.uncertain.all = true;
            self.admin.all = true;
        }
    }
}

fn index_of<'a>(o: &'a StoredObject, key: &str) -> Option<&'a str> {
    o.index.get(key)?.first().map(String::as_str)
}

fn id_in(o: &StoredObject, key: &str) -> Option<ObjectId> {
    index_of(o, key)?.parse().ok()
}

/// A delegation that cannot be read: could its right be `Manage`, and which group would hold it?
/// The index and the content are both looked at; any doubt counts against us (fail closed).
fn doubtful_delegation(o: &StoredObject, doubt: &mut Doubt, admin: &mut BTreeSet<ObjectId>) {
    let right = |s: Option<&str>| s.and_then(Right::parse);
    let content = |k: &str| o.content.get(k).and_then(|v| v.as_str());
    doubt.unreadable.push(o.envelope.id);
    let harmless = |r: Option<Right>| r.is_some_and(|r| r != Right::Manage);
    if harmless(right(index_of(o, "right"))) && harmless(right(content("fs:right"))) {
        return; // whatever else is wrong with it, it is no admin right
    }
    let mut holders: Vec<ObjectId> = vec![];
    holders.extend(id_in(o, "holder").filter(|_| index_of(o, "holder_kind") == Some("group")));
    holders.extend(content("subject").and_then(|s| s.parse::<ObjectId>().ok()));
    if holders.is_empty() {
        if index_of(o, "holder_kind") != Some("person") {
            doubt.admin.all = true; // the holder cannot be told
        }
        return;
    }
    admin.extend(holders);
}

impl GroupRepo {
    pub(super) async fn directory_state(&self) -> Result<Wanted> {
        let found = self
            .reads
            .query(&Query::new().in_context(CONTEXT).ordered(Order::NameAsc))
            .await?;
        let (mut groups, mut bad_groups) = (vec![], vec![]);
        let (mut memberships, mut delegations, mut strays) = (vec![], vec![], vec![]);
        for o in found {
            match o.envelope.object_type.as_str() {
                GROUP_TYPE => match Group::from_content(o.envelope.clone(), &o.content) {
                    Some(g) => groups.push(g),
                    None => bad_groups.push((o.envelope.id, Group::parents_in(&o.content))),
                },
                MEMBERSHIP_TYPE => memberships.push(o),
                DELEGATION_TYPE => delegations.push(o),
                INVITATION_TYPE | PARENT_REQUEST_TYPE | LEDGER_TYPE => {}
                _ => strays.push(o),
            }
        }
        let by_id: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g)).collect();
        let graph = Graph::from_groups(&groups);
        let looped = graph.cyclic_nodes();
        let mut doubt = Doubt::default();
        let bad_ids: BTreeSet<_> = bad_groups.iter().map(|(id, _)| *id).collect();
        // A group that cannot be read: what is above it loses what comes from below it.
        for (id, parents) in bad_groups {
            doubt.unreadable.push(id);
            doubt.uncertain.groups.insert(id);
            match parents {
                Some(ps) => ps.into_iter().for_each(|p| doubt.above(&graph, p)),
                None => doubt.uncertain.all = true,
            }
        }
        // A parent that is nowhere is reported, and any group may be missing this child's people.
        for g in &groups {
            if g.parents
                .iter()
                .any(|p| !by_id.contains_key(p) && !bad_ids.contains(p))
            {
                // which group it was meant to be in cannot be told
                doubt.unreadable.push(g.id);
                doubt.uncertain.all = true;
            }
        }
        let mut people: People = BTreeMap::new();
        for o in &memberships {
            match codec::membership(o) {
                Some(m) => match m.member {
                    Member::Person(p) => {
                        people.entry(m.group).or_default().insert((p, m.role));
                    }
                    Member::Group(_) => doubt.touches(&graph, o.envelope.id, id_in(o, "group")),
                },
                None => doubt.touches(&graph, o.envelope.id, id_in(o, "group")),
            }
        }
        let mut strict: BTreeSet<ObjectId> = BTreeSet::new();
        for o in &delegations {
            match codec::delegation(o) {
                Some(d) => {
                    if let (Right::Manage, Member::Group(h)) = (d.right, d.holder) {
                        strict.insert(h);
                    }
                }
                None => doubtful_delegation(o, &mut doubt, &mut strict),
            }
        }
        for o in &strays {
            doubt.touches(&graph, o.envelope.id, id_in(o, "group"));
        }
        for (group, roles) in &people {
            if roles.iter().any(|(_, role)| self.is_admin_role(role)) {
                strict.insert(*group);
            }
        }
        strict.extend(doubt.admin.groups.iter().copied());
        let flows = right_flow::settings(&groups);
        let build = Build {
            by_id: &by_id,
            graph: &graph,
            flows: &flows,
            looped: &looped,
            people: &people,
        };
        let mut unreadable = std::mem::take(&mut doubt.unreadable);
        unreadable.sort();
        unreadable.dedup();
        Ok(Wanted {
            groups: build.groups(),
            looped: looped.into_iter().collect(),
            unreadable,
            uncertain: doubt.uncertain,
            admin: AdminRule {
                strict,
                lenient_all: doubt.admin.all,
            },
        })
    }
}

/// The inputs of the pass that builds the directory groups.
struct Build<'a> {
    by_id: &'a BTreeMap<ObjectId, &'a Group>,
    graph: &'a Graph,
    flows: &'a BTreeMap<ObjectId, RightFlow>,
    looped: &'a BTreeSet<ObjectId>,
    people: &'a People,
}

impl Build<'_> {
    fn direct(&self, id: &ObjectId) -> BTreeSet<String> {
        self.people
            .get(id)
            .map(|s| s.iter().map(|(p, _)| p.clone()).collect())
            .unwrap_or_default()
    }

    /// Children first: each group with the children nested in it, and the people of the others
    /// (their whole subtrees) listed in it.
    fn groups(&self) -> Vec<DirectoryGroup> {
        let mut out = vec![];
        for id in self.graph.children_first() {
            let Some(g) = self.by_id.get(&id) else {
                continue;
            };
            let mut members = self.direct(&id);
            let mut nested = vec![];
            for child in self.graph.children(id) {
                let edge_ok = right_flow::directory_nests(self.flows, *child, id)
                    && !self.looped.contains(child)
                    && !self.looped.contains(&id);
                if edge_ok {
                    nested.push(*child);
                } else {
                    for below in std::iter::once(*child).chain(self.graph.descendants(*child)) {
                        members.extend(self.direct(&below));
                    }
                }
            }
            out.push(
                DirectoryGroup {
                    id,
                    name: g.name.clone(),
                    members: members.into_iter().collect(),
                    nested,
                }
                .normalised(),
            );
        }
        out
    }
}
