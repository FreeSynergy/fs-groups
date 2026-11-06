//! The wanted state of a directory, worked out from the groups in one pass.
//!
//! Pattern: Specification over the in-memory graph — option B (ADR 0056, provisional): a child is
//! nested only where `right_flow::directory_nests` says so and nothing forbids it, every other child
//! is listed flat in the parent. Nothing forbids it unless the parent carries an administrative right
//! and the child's directory closure holds somebody who is an agent or not known to be a human.
//! Such people are never written into a group that carries an administrative right, however they
//! would get there (listed, flat from a child, nested through a child), and are named in the result.

use std::collections::{BTreeMap, BTreeSet};

use fs_store::{ObjectId, Query};

use super::codec;
use super::consts::{CONTEXT, DELEGATION_TYPE, MEMBERSHIP_TYPE};
use super::handle::GroupRepo;
use crate::error::Result;
use crate::graph::Graph;
use crate::mirror::{Scope, Wanted};
use crate::model::{Member, Right};
use crate::right_flow;
use crate::traits::DirectoryGroup;
use fs_store::StoredObject;

impl GroupRepo {
    /// The groups that carry an administrative right: they hold `Manage` on something, or a member
    /// has a configured admin role in them. **Fail closed:** a delegation that cannot be read, and
    /// whose right may be `Manage`, makes its holder group administrative (all groups, if the holder
    /// cannot be told); so does a membership that cannot be read, for its group.
    async fn admin_groups(
        &self,
        people: &People,
        extra: &mut Scope,
        unreadable: &mut Vec<ObjectId>,
    ) -> Result<BTreeSet<ObjectId>> {
        let q = Query::new().of_type(DELEGATION_TYPE).in_context(CONTEXT);
        let mut out = BTreeSet::new();
        for o in self.reads.query(&q).await? {
            if let Some(d) = codec::delegation(&o) {
                if let (Right::Manage, Member::Group(h)) = (d.right, d.holder) {
                    out.insert(h);
                }
                continue;
            }
            unreadable.push(o.envelope.id);
            let right = index_of(&o, "right").and_then(Right::parse);
            if right.is_some_and(|r| r != Right::Manage) {
                continue; // whatever else is wrong with it, it is no admin right
            }
            match (index_of(&o, "holder_kind"), id_in(&o, "holder")) {
                (Some("group"), Some(h)) => {
                    extra.groups.insert(h);
                }
                (Some("person"), _) => {}
                _ => extra.all = true,
            }
        }
        for (group, held) in people {
            if held.iter().any(|(_, role)| self.is_admin_role(role)) {
                out.insert(*group);
            }
        }
        Ok(out)
    }

    /// Every membership, so that one that cannot be read is noticed: who is in which group with which
    /// role, and what an unreadable one touches.
    async fn read_people(
        &self,
        graph: &Graph,
        scopes: &mut Scopes,
        unreadable: &mut Vec<ObjectId>,
    ) -> Result<People> {
        let q = Query::new().of_type(MEMBERSHIP_TYPE).in_context(CONTEXT);
        let mut people: People = BTreeMap::new();
        for o in self.reads.query(&q).await? {
            if let Some(m) = codec::membership(&o) {
                if let Member::Person(p) = m.member {
                    people.entry(m.group).or_default().insert((p, m.role));
                }
            } else {
                unreadable.push(o.envelope.id);
                if let Some(g) = id_in(&o, "group") {
                    scopes.touch(graph, g);
                    scopes.admin.groups.insert(g);
                } else {
                    scopes.uncertain.all = true;
                    scopes.admin.all = true;
                }
            }
        }
        Ok(people)
    }

    pub(super) async fn directory_state(&self) -> Result<Wanted> {
        let (groups, bad_groups) = self.read_groups().await?;
        let by_id: BTreeMap<_, _> = groups.iter().map(|g| (g.id, g)).collect();
        let graph = Graph::from_groups(&groups);
        let looped = graph.cyclic_nodes();
        let (mut unreadable, mut scopes) = (vec![], Scopes::default());
        // A group that cannot be read: what is above it loses what comes from below it.
        for u in bad_groups {
            unreadable.push(u.id);
            scopes.uncertain.groups.insert(u.id);
            match u.parents {
                Some(parents) => parents.into_iter().for_each(|p| scopes.touch(&graph, p)),
                None => scopes.uncertain.all = true,
            }
        }
        let people = self
            .read_people(&graph, &mut scopes, &mut unreadable)
            .await?;
        let mut admin = self
            .admin_groups(&people, &mut scopes.admin, &mut unreadable)
            .await?;
        admin.extend(scopes.admin.groups.iter().copied());
        if scopes.admin.all {
            admin.extend(by_id.keys().copied());
        }
        let admin_scope = Scope {
            all: scopes.admin.all,
            groups: admin,
        };
        // who is not known to be a human; only asked when an administrative group exists
        let mut bad = BTreeSet::new();
        if !admin_scope.is_empty() {
            let everyone: BTreeSet<_> = people.values().flatten().map(|(p, _)| p.clone()).collect();
            for p in everyone {
                if !self.is_known_human(&p).await? {
                    bad.insert(p);
                }
            }
        }
        let flows = right_flow::settings(&groups);
        let build = Build {
            by_id: &by_id,
            graph: &graph,
            flows: &flows,
            looped: &looped,
            people: &people,
            admin: &admin_scope,
            bad: &bad,
        };
        let (out, left_out) = build.groups();
        unreadable.sort();
        unreadable.dedup();
        Ok(Wanted {
            groups: out,
            left_out,
            looped: looped.into_iter().collect(),
            unreadable,
            uncertain: scopes.uncertain,
            admin: admin_scope,
        })
    }
}

type People = BTreeMap<ObjectId, BTreeSet<(String, String)>>;

/// What unreadable objects touch: the groups whose wanted list may be short, and the groups that
/// count as administrative because of an object that cannot be read.
#[derive(Default)]
struct Scopes {
    uncertain: Scope,
    admin: Scope,
}

impl Scopes {
    /// `g` and everything above it: they list the people of `g`.
    fn touch(&mut self, graph: &Graph, g: ObjectId) {
        self.uncertain.groups.insert(g);
        self.uncertain.groups.extend(graph.ancestors(g));
    }
}

/// The inputs of the pass that builds the directory groups.
struct Build<'a> {
    by_id: &'a BTreeMap<ObjectId, &'a crate::model::Group>,
    graph: &'a Graph,
    flows: &'a BTreeMap<ObjectId, crate::model::RightFlow>,
    looped: &'a BTreeSet<ObjectId>,
    people: &'a People,
    admin: &'a Scope,
    bad: &'a BTreeSet<String>,
}

impl Build<'_> {
    fn direct(&self, id: &ObjectId) -> BTreeSet<String> {
        self.people
            .get(id)
            .map(|s| s.iter().map(|(p, _)| p.clone()).collect())
            .unwrap_or_default()
    }

    /// Children first: the groups with their nested children, and who was left out where.
    fn groups(&self) -> (Vec<DirectoryGroup>, Vec<(ObjectId, Vec<String>)>) {
        let mut closure: BTreeMap<ObjectId, BTreeSet<String>> = BTreeMap::new();
        let (mut out, mut left_out) = (vec![], vec![]);
        for id in self.graph.children_first() {
            let Some(g) = self.by_id.get(&id) else {
                continue;
            };
            let is_admin = self.admin.covers(id);
            let mut members = self.direct(&id);
            let mut nested = vec![];
            for child in self.graph.children(id) {
                let edge_ok = right_flow::directory_nests(self.flows, *child, id)
                    && !self.looped.contains(child)
                    && !self.looped.contains(&id);
                let clean = !is_admin
                    || closure
                        .get(child)
                        .is_some_and(|c| c.iter().all(|p| !self.bad.contains(p)));
                if edge_ok && clean && closure.contains_key(child) {
                    nested.push(*child);
                } else {
                    for below in std::iter::once(*child).chain(self.graph.descendants(*child)) {
                        members.extend(self.direct(&below));
                    }
                }
            }
            if is_admin {
                let gone: Vec<_> = members
                    .iter()
                    .filter(|p| self.bad.contains(*p))
                    .cloned()
                    .collect();
                members.retain(|p| !self.bad.contains(p));
                if !gone.is_empty() {
                    left_out.push((id, gone));
                }
            }
            let mut reach = members.clone();
            for c in &nested {
                reach.extend(closure.get(c).into_iter().flatten().cloned());
            }
            closure.insert(id, reach);
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
        (out, left_out)
    }
}

fn index_of<'a>(o: &'a StoredObject, key: &str) -> Option<&'a str> {
    o.index.get(key)?.first().map(String::as_str)
}

fn id_in(o: &StoredObject, key: &str) -> Option<ObjectId> {
    index_of(o, key)?.parse().ok()
}
