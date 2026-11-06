//! The rule "an agent is never admin" as one pure function over the final plan.
//!
//! Pattern: Specification (pure) — [`enforce`] is the only place that knows the rule. It looks at
//! the **planned** closure of every administrative group (what the directory will hold after the run,
//! wanted, held still or retained alike) and removes anybody the rule bars: a direct listing is
//! dropped, a nested edge whose child closure holds one is replaced by a flat listing without them.
//! It repeats until nothing changes. What it is given is the plan and the host's verdicts, nothing
//! else, so it can be tested on random states.

use std::collections::{BTreeMap, BTreeSet};

use fs_store::ObjectId;

use super::wanted::{AdminRule, Bar, Verdict};
use crate::traits::DirectoryGroup;

/// What [`enforce`] reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Enforced {
    /// `(group, people)` that are really out of the group's closure because of the rule.
    pub left_out: Vec<(ObjectId, Vec<String>)>,
    /// `(group, people)` in a group that may be administrative (an unreadable object could not be
    /// placed) whom the host could not vouch for; they stay and are reported.
    pub unchecked: Vec<(ObjectId, Vec<String>)>,
}

fn verdict_of(verdicts: &BTreeMap<String, Verdict>, person: &str) -> Verdict {
    verdicts.get(person).copied().unwrap_or(Verdict::Unknown)
}

/// Everybody listed in `id` or in what is nested below it in `plan`.
pub fn closure(plan: &BTreeMap<ObjectId, DirectoryGroup>, id: ObjectId) -> BTreeSet<String> {
    let (mut seen, mut todo, mut out) = (BTreeSet::new(), vec![id], BTreeSet::new());
    while let Some(cur) = todo.pop() {
        if let Some(g) = plan.get(&cur).filter(|_| seen.insert(cur)) {
            out.extend(g.members.iter().cloned());
            todo.extend(g.nested.iter().copied());
        }
    }
    out
}

/// Make `plan` obey the rule, and say what that cost.
pub fn enforce(
    plan: &mut BTreeMap<ObjectId, DirectoryGroup>,
    rule: &AdminRule,
    verdicts: &BTreeMap<String, Verdict>,
) -> Enforced {
    let barred =
        |group: ObjectId, person: &str| rule.bars(group, verdict_of(verdicts, person)) == Bar::Out;
    let ids: Vec<ObjectId> = plan.keys().copied().collect();
    let mut candidates: BTreeMap<ObjectId, BTreeSet<String>> = BTreeMap::new();
    // every change removes a person or turns a nested edge into a listing, and an edge is never
    // added back, so this ends
    let mut changed = true;
    while changed {
        changed = false;
        for id in &ids {
            // edges whose child closure holds somebody barred in this group become listings
            let Some(nested) = plan.get(id).map(|g| g.nested.clone()) else {
                continue;
            };
            for child in nested {
                let below = closure(plan, child);
                let bad: BTreeSet<String> =
                    below.iter().filter(|p| barred(*id, p)).cloned().collect();
                if !bad.is_empty() {
                    let Some(group) = plan.get_mut(id) else {
                        continue;
                    };
                    group.nested.retain(|c| *c != child);
                    group.members.extend(below.difference(&bad).cloned());
                    group.members.sort();
                    group.members.dedup();
                    candidates.entry(*id).or_default().extend(bad);
                    changed = true;
                }
            }
            let Some(group) = plan.get_mut(id) else {
                continue;
            };
            let before = group.members.len();
            let (keep, out): (Vec<_>, Vec<_>) =
                group.members.iter().cloned().partition(|p| !barred(*id, p));
            group.members = keep;
            if group.members.len() != before {
                candidates.entry(*id).or_default().extend(out);
                changed = true;
            }
        }
    }
    let mut result = Enforced::default();
    for id in ids {
        let now = closure(plan, id);
        let gone: Vec<String> = candidates
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| !now.contains(p))
            .collect();
        if !gone.is_empty() {
            result.left_out.push((id, gone));
        }
        let doubtful: Vec<String> = now
            .into_iter()
            .filter(|p| rule.bars(id, verdict_of(verdicts, p)) == Bar::Unchecked)
            .collect();
        if !doubtful.is_empty() {
            result.unchecked.push((id, doubtful));
        }
    }
    result
}
