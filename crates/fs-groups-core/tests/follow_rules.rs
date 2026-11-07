//! The pure rules of following (Specification), without a store.

use fs_groups_core::follow_rules::{from_origin, local_group, may_pass_on, normalise, within};
use fs_groups_core::{Offer, Right, Source, SourceState};
use fs_store::ObjectId;

fn source(pass_on: bool, state: SourceState) -> Source {
    Source {
        id: ObjectId::new(),
        group: ObjectId::new(),
        offer: Offer {
            delivery: ObjectId::new(),
            source: "https://a.example/groups/x".into(),
            recipient: "https://b.example/groups/y".into(),
            rights: vec![Right::Read],
            pass_on,
            origin: "https://a.example/groups/x".into(),
        },
        sender: "a.example".into(),
        state,
    }
}

#[test]
fn rights_are_sorted_without_repeats_and_never_manage() {
    let r = normalise(&[Right::AddMembers, Right::Read, Right::Read]).unwrap();
    assert_eq!(r, vec![Right::Read, Right::AddMembers]);
    assert!(normalise(&[]).is_err());
    assert!(normalise(&[Right::Read, Right::Manage]).is_err());
}

#[test]
fn within_means_nothing_beyond_what_was_given() {
    assert!(within(&[Right::Read, Right::AddMembers], &[Right::Read]));
    assert!(!within(&[Right::Read], &[Right::Read, Right::AddMembers]));
    assert!(within(&[Right::Manage], &[Right::RemoveMembers]));
    assert!(within(&[Right::Read], &[]));
}

#[test]
fn passing_on_needs_an_active_source_that_allows_it() {
    assert!(may_pass_on(&source(true, SourceState::Active)));
    assert!(!may_pass_on(&source(false, SourceState::Active)));
    assert!(!may_pass_on(&source(true, SourceState::Withdrawn)));
}

#[test]
fn an_offer_must_lie_at_the_named_origin() {
    assert!(from_origin("a.example", "https://a.example/groups/x"));
    assert!(from_origin(" A.Example ", "https://a.example/groups/x"));
    assert!(!from_origin("evil.example", "https://a.example/groups/x"));
    assert!(!from_origin("a.example", "https://a.example.evil/groups/x"));
    assert!(!from_origin("", "https://a.example/groups/x"));
    assert!(!from_origin("a.example", "http://a.example/groups/x"));
}

#[test]
fn a_local_group_is_read_from_its_iri() {
    let id = ObjectId::new();
    assert_eq!(
        local_group("b.example", &format!("https://b.example/groups/{id}")),
        Some(id)
    );
    assert_eq!(
        local_group("b.example", &format!("https://c.example/groups/{id}")),
        None
    );
    assert_eq!(
        local_group("b.example", "https://b.example/groups/nope"),
        None
    );
}
