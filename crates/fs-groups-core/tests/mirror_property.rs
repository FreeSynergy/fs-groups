//! The invariant of ADR 0056 in the mirror, on random states: **no agent and no unknown person is in
//! the closure of an administrative group after any prefix of the writes of a run**, whatever is
//! unreadable and whichever writes the directory refuses. A violation that was there before the run
//! (somebody marked as an agent later) may only shrink, never grow; with nothing refused the run
//! ends without any.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{ME, Recording, link, person, setup};
use fs_groups_core::{
    AiMarking, DirectoryGroup, Group, GroupRepo, Member, Mirror, Right, RightFlow,
};
use fs_objects::{ObjectId, Store};

/// The host: listed people are agents, others listed are unknown, everybody else is human.
#[derive(Default)]
struct Marks {
    agents: Mutex<BTreeSet<String>>,
    unknown: Mutex<BTreeSet<String>>,
}

#[async_trait]
impl AiMarking for Marks {
    async fn is_ai(&self, p: &str) -> fs_groups_core::Result<Option<bool>> {
        if self.unknown.lock().unwrap().contains(p) {
            return Ok(None);
        }
        Ok(Some(self.agents.lock().unwrap().contains(p)))
    }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap() % n
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.next(100) < percent
    }
}

type State = BTreeMap<ObjectId, DirectoryGroup>;

fn closure(state: &State, id: ObjectId) -> BTreeSet<String> {
    let (mut seen, mut todo, mut out) = (BTreeSet::new(), vec![id], BTreeSet::new());
    while let Some(cur) = todo.pop() {
        if let Some(g) = state.get(&cur).filter(|_| seen.insert(cur)) {
            out.extend(g.members.iter().cloned());
            todo.extend(g.nested.iter().copied());
        }
    }
    out
}

fn violations(state: &State, admin: &[ObjectId], marks: &Marks) -> BTreeSet<(ObjectId, String)> {
    let (agents, unknown) = (marks.agents.lock().unwrap(), marks.unknown.lock().unwrap());
    let mut out = BTreeSet::new();
    for a in admin {
        for p in closure(state, *a) {
            if agents.contains(&p) || unknown.contains(&p) {
                out.insert((*a, p));
            }
        }
    }
    out
}

async fn corrupt(store: &Arc<dyn Store>, rng: &mut Lcg, groups: &[Group], repo: &GroupRepo) {
    let g = &groups[rng.next(groups.len())];
    let Ok(held) = repo.direct(g.id).await else {
        return;
    };
    match rng.next(3) {
        0 => {
            let mut o = store.get(g.id).await.unwrap().unwrap();
            o.content.as_object_mut().unwrap().remove("name");
            store.put(o, ME).await.unwrap();
        }
        1 if !held.is_empty() => {
            let m = &held[rng.next(held.len())];
            let mut o = store.get(m.id).await.unwrap().unwrap();
            o.index.remove(if rng.chance(50) {
                "member_kind"
            } else {
                "group"
            });
            store.put(o, ME).await.unwrap();
        }
        _ => {
            if let Ok(ds) = repo.delegations(g.id).await {
                if let Some(d) = ds.first() {
                    let mut o = store.get(d.id).await.unwrap().unwrap();
                    o.index.remove("right");
                    store.put(o, ME).await.unwrap();
                }
            }
        }
    }
}

/// Markings, memberships and removals change between the two syncs.
async fn churn(
    repo: &GroupRepo,
    groups: &[Group],
    people: &[String],
    marks: &Marks,
    rng: &mut Lcg,
) {
    // things change: markings, memberships, damage, refusals
    for p in people {
        if rng.chance(25) {
            marks.agents.lock().unwrap().insert(p.clone());
        } else if rng.chance(15) {
            marks.unknown.lock().unwrap().insert(p.clone());
        }
    }
    for g in groups {
        let p = &people[rng.next(people.len())];
        if rng.chance(40) {
            let _ = repo.add(g.id, Member::Person(p.clone()), "").await;
        } else if rng.chance(30) {
            if let Some(m) = repo.direct(g.id).await.unwrap().first() {
                repo.remove(m.id).await.unwrap();
            }
        }
    }
}

#[tokio::test]
async fn no_prefix_of_any_run_puts_an_agent_or_unknown_into_an_administrative_closure() {
    let people: Vec<String> = (0..6).map(|n| person(&format!("p{n}"))).collect();
    let (mut shrunk, mut with_refusals, mut with_violations) = (0, 0, 0);
    for seed in 0..60u64 {
        let mut rng = Lcg(seed * 7919 + 1);
        let (repo, store) = setup().await;
        let marks = Arc::new(Marks::default());
        let repo = repo
            .with_ai_marking(marks.clone())
            .with_admin_roles(["owner"])
            .unwrap();
        let n = 3 + rng.next(4);
        let mut groups: Vec<Group> = vec![];
        for i in 0..n {
            let g = repo.create(&format!("g{i}"), "t").await.unwrap();
            let flow = if rng.chance(70) {
                RightFlow::Up
            } else {
                RightFlow::None
            };
            groups.push(repo.set_right_flow(g.id, flow).await.unwrap());
        }
        for i in 1..n {
            for j in 0..i {
                if rng.chance(35) {
                    link(&repo, groups[i].id, groups[j].id).await;
                }
            }
        }
        let mut admin = vec![];
        for g in &groups {
            if rng.chance(35) {
                let target = repo.create("target", "t").await.unwrap();
                repo.grant(target.id, Member::Group(g.id), Right::Manage)
                    .await
                    .unwrap();
                admin.push(g.id);
            }
        }
        for g in &groups {
            for p in &people {
                if rng.chance(30) {
                    repo.add(g.id, Member::Person(p.clone()), "").await.unwrap();
                }
            }
        }
        let rec = Recording::default();
        repo.sync_to(&rec).await.unwrap();
        churn(&repo, &groups, &people, &marks, &mut rng).await;
        for _ in 0..rng.next(3) {
            corrupt(&store, &mut rng, &groups, &repo).await;
        }
        let refusals: Vec<_> = groups
            .iter()
            .filter(|_| rng.chance(25))
            .map(|g| g.id)
            .collect();
        rec.refuse.lock().unwrap().extend(refusals.iter().copied());
        rec.start();
        let report = repo.sync_to(&rec).await.unwrap();
        let states = rec.states.lock().unwrap().clone();
        let before = violations(&states[0], &admin, &marks);
        for (i, state) in states.iter().enumerate() {
            let now = violations(state, &admin, &marks);
            assert!(
                now.is_subset(&before),
                "seed {seed}, state {i}: new violations {:?}",
                now.difference(&before).collect::<Vec<_>>()
            );
        }
        let last = violations(states.last().unwrap(), &admin, &marks);
        if refusals.is_empty() {
            assert!(
                last.is_empty(),
                "seed {seed}: left {last:?}\nreport {report:?}\nfinal {:#?}",
                states.last().unwrap()
            );
        } else {
            with_refusals += 1;
        }
        if !before.is_empty() {
            with_violations += 1;
            shrunk += usize::from(last.len() < before.len());
        }
    }
    assert!(
        with_violations > 5 && shrunk > 3 && with_refusals > 5,
        "{with_violations} {shrunk} {with_refusals}"
    );
}
