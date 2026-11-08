//! The pure graph: walks, paths and the order for a directory.

use fs_groups_core::{Graph, Group};
use fs_objects::{Envelope, ObjectId};

fn group(parents: &[ObjectId]) -> Group {
    let envelope = Envelope::new("Group", "x");
    let id = envelope.id;
    Group {
        id,
        iri: String::new(),
        name: id.to_string(),
        kind: String::new(),
        parents: parents.to_vec(),
        right_flow: fs_groups_core::RightFlow::default(),
        envelope,
    }
}

#[test]
fn children_come_before_their_parents() {
    let top = group(&[]);
    let mid = group(&[top.id]);
    let leaf = group(&[mid.id, top.id]);
    let g = Graph::from_groups(&[top.clone(), mid.clone(), leaf.clone()]);
    assert_eq!(g.children_first(), [leaf.id, mid.id, top.id]);
}

#[test]
fn unknown_parents_are_ignored_and_cycles_do_not_hang() {
    let a = group(&[]);
    let mut b = group(&[a.id, ObjectId::new()]);
    let mut a2 = a.clone();
    a2.parents = vec![b.id];
    b.parents.push(b.id); // even a self-parent in bad data
    let g = Graph::from_groups(&[a2.clone(), b.clone()]);
    assert_eq!(g.children_first().len(), 2);
    assert_eq!(g.ancestors(a2.id), [b.id]);
    assert_eq!(g.descendants(a2.id), [b.id]);
    assert_eq!(g.path_up(a2.id, b.id), Some(vec![a2.id, b.id]));
}
