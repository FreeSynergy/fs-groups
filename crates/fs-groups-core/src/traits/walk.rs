//! Breadth-first walk over the group graph, each group once.
//!
//! Pattern: Iterator / Visitor — the one traversal behind `ancestors` and `descendants`. A visited set
//! makes it end on any data, even a cycle that got in from outside.

use std::collections::{BTreeSet, VecDeque};

use fs_store::ObjectId;

use super::Groups;
use crate::error::Result;

/// Groups reachable from `start` by `parents` (`up`) or `children`, nearest first, `start` excluded.
pub(super) async fn walk<G: Groups + ?Sized>(
    g: &G,
    start: ObjectId,
    up: bool,
) -> Result<Vec<ObjectId>> {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut out = vec![];
    while let Some(cur) = queue.pop_front() {
        let next = if up {
            g.parents(cur).await?
        } else {
            g.children(cur).await?
        };
        for n in next {
            if seen.insert(n) {
                out.push(n);
                queue.push_back(n);
            }
        }
    }
    Ok(out)
}
