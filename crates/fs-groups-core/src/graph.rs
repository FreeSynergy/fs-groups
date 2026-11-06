//! The group graph in memory: ancestors, descendants with their paths, cycle paths, order.
//!
//! Pattern: Visitor / Iterator — pure breadth-first walks over a parent map with a visited set, so
//! every walk ends on any data, even a cycle that got in from outside. No storage here.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fs_store::ObjectId;

use crate::model::Group;

/// Parents and children of all groups, loaded once.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    parents: BTreeMap<ObjectId, Vec<ObjectId>>,
    children: BTreeMap<ObjectId, Vec<ObjectId>>,
}

impl Graph {
    /// Build from groups. A parent id that is not among them is ignored.
    pub fn from_groups(groups: &[Group]) -> Self {
        let known: BTreeSet<_> = groups.iter().map(|g| g.id).collect();
        let mut g = Graph::default();
        for group in groups {
            g.parents.entry(group.id).or_default();
            g.children.entry(group.id).or_default();
        }
        for group in groups {
            for p in group.parents.iter().filter(|p| known.contains(p)) {
                g.parents.entry(group.id).or_default().push(*p);
                g.children.entry(*p).or_default().push(group.id);
            }
        }
        g
    }

    /// Whether the group is in the graph.
    pub fn contains(&self, id: ObjectId) -> bool {
        self.parents.contains_key(&id)
    }

    /// Direct parents.
    pub fn parents(&self, id: ObjectId) -> &[ObjectId] {
        self.parents.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Direct children.
    pub fn children(&self, id: ObjectId) -> &[ObjectId] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }

    /// All ancestors, nearest first, each once, `start` excluded.
    pub fn ancestors(&self, start: ObjectId) -> Vec<ObjectId> {
        self.reach(start, true)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    /// All descendants, nearest first, each once, `start` excluded.
    pub fn descendants(&self, start: ObjectId) -> Vec<ObjectId> {
        self.reach(start, false)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    /// Every descendant with the path from `start` to it (both ends included), shortest first.
    pub fn descendants_with_paths(&self, start: ObjectId) -> Vec<(ObjectId, Vec<ObjectId>)> {
        self.reach(start, false)
    }

    /// The chain of parents from `from` up to `to`, both included, if `to` is an ancestor of `from`
    /// (or `from` itself). Used to explain a refused cycle.
    pub fn path_up(&self, from: ObjectId, to: ObjectId) -> Option<Vec<ObjectId>> {
        if from == to {
            return Some(vec![from]);
        }
        self.reach(from, true)
            .into_iter()
            .find(|(id, _)| *id == to)
            .map(|(_, path)| path)
    }

    /// Children before their parents, so a directory can refer to a nested group that exists.
    /// Groups on a cycle (which this core refuses) come last, in id order.
    pub fn children_first(&self) -> Vec<ObjectId> {
        let mut waiting: BTreeMap<_, _> =
            self.children.iter().map(|(k, v)| (*k, v.len())).collect();
        let mut ready: VecDeque<_> = waiting
            .iter()
            .filter(|(_, n)| **n == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut out = vec![];
        while let Some(id) = ready.pop_front() {
            out.push(id);
            for p in self.parents(id) {
                if let Some(n) = waiting.get_mut(p) {
                    *n -= 1;
                    if *n == 0 {
                        ready.push_back(*p);
                    }
                }
            }
        }
        let done: BTreeSet<_> = out.iter().copied().collect();
        out.extend(self.parents.keys().filter(|k| !done.contains(k)));
        out
    }

    /// The groups that lie on a cycle or between cycles. Empty for good data. Used to refuse an
    /// answer that would rest on a loop. Found by peeling off, in turn, the groups without
    /// children and the groups without parents; what cannot be peeled is looped.
    pub fn cyclic_nodes(&self) -> BTreeSet<ObjectId> {
        let mut alive: BTreeSet<_> = self.parents.keys().copied().collect();
        peel(&mut alive, |id| self.children(id), |id| self.parents(id));
        peel(&mut alive, |id| self.parents(id), |id| self.children(id));
        alive
    }

    fn reach(&self, start: ObjectId, up: bool) -> Vec<(ObjectId, Vec<ObjectId>)> {
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([(start, vec![start])]);
        let mut out = vec![];
        while let Some((cur, path)) = queue.pop_front() {
            let next = if up {
                self.parents(cur)
            } else {
                self.children(cur)
            };
            for n in next.iter().filter(|n| seen.insert(**n)) {
                let mut p = path.clone();
                p.push(*n);
                out.push((*n, p.clone()));
                queue.push_back((*n, p));
            }
        }
        out
    }
}

/// Remove from `alive` every group all of whose `out` neighbours are gone, repeatedly.
fn peel<'a>(
    alive: &mut BTreeSet<ObjectId>,
    out: impl Fn(ObjectId) -> &'a [ObjectId],
    back: impl Fn(ObjectId) -> &'a [ObjectId],
) {
    let mut waiting: BTreeMap<_, _> = alive
        .iter()
        .map(|id| (*id, out(*id).iter().filter(|n| alive.contains(n)).count()))
        .collect();
    let mut ready: VecDeque<_> = waiting
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    while let Some(id) = ready.pop_front() {
        alive.remove(&id);
        for b in back(id) {
            if let Some(n) = waiting.get_mut(b) {
                *n -= 1;
                if *n == 0 {
                    ready.push_back(*b);
                }
            }
        }
    }
}
