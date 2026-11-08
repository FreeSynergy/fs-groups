//! The bridge against a role book: short id, full IRI, humans, unknowns, errors.

use std::sync::Arc;

use async_trait::async_trait;
use fs_groups_core::AiMarking as _;
use fs_groups_people::PeopleMarking;
use fs_objects::ObjectId;
use fs_people_core::{AI_ROLE, AI_ROLE_IRI, RoleAssignment, RoleBook};
use serde_json::Map;

/// A role book that holds the roles of two people and fails for a third.
struct Book {
    ada: ObjectId,
    bot: ObjectId,
    full: ObjectId,
    broken: ObjectId,
}

fn assignment(person: ObjectId, role: &str) -> RoleAssignment {
    RoleAssignment {
        id: ObjectId::new(),
        person,
        role: role.into(),
        scope: None,
        details: Map::new(),
    }
}

#[async_trait]
impl RoleBook for Book {
    async fn roles_of(&self, p: ObjectId) -> fs_people_core::Result<Vec<RoleAssignment>> {
        if p == self.ada {
            Ok(vec![assignment(p, "member")])
        } else if p == self.bot {
            Ok(vec![assignment(p, "member"), assignment(p, AI_ROLE)])
        } else if p == self.full {
            Ok(vec![assignment(p, AI_ROLE_IRI)])
        } else if p == self.broken {
            Err(fs_people_core::Error::Conflict("down".into()))
        } else {
            Ok(vec![])
        }
    }
}

fn iri(p: ObjectId) -> String {
    format!("https://example.org/people/{p}")
}

fn setup() -> (PeopleMarking, Book) {
    let ids = || ObjectId::new();
    let book = || Book {
        ada: ids(),
        bot: ids(),
        full: ids(),
        broken: ids(),
    };
    let b = book();
    let copy = Book {
        ada: b.ada,
        bot: b.bot,
        full: b.full,
        broken: b.broken,
    };
    (PeopleMarking::new(Arc::new(b)), copy)
}

#[tokio::test]
async fn the_short_id_and_the_full_iri_both_mark_an_ai() {
    let (m, b) = setup();
    assert_eq!(
        m.is_ai(&iri(b.bot)).await.unwrap(),
        Some(true),
        "short id `ai`"
    );
    assert_eq!(m.is_ai(&iri(b.full)).await.unwrap(), Some(true), "full IRI");
}

#[tokio::test]
async fn a_person_with_other_roles_is_known_human() {
    let (m, b) = setup();
    assert_eq!(m.is_ai(&iri(b.ada)).await.unwrap(), Some(false));
    assert_eq!(
        m.is_ai(&format!("{}/", iri(b.ada))).await.unwrap(),
        Some(false),
        "a trailing slash is the same person"
    );
}

#[tokio::test]
async fn unknown_is_no_answer_never_human() {
    let (m, _) = setup();
    assert_eq!(
        m.is_ai(&iri(ObjectId::new())).await.unwrap(),
        None,
        "no roles"
    );
    for odd in ["", "https://example.org/people/not-an-id", "ada"] {
        assert_eq!(m.is_ai(odd).await.unwrap(), None, "{odd:?}");
    }
}

#[tokio::test]
async fn an_error_of_the_book_is_an_error() {
    let (m, b) = setup();
    assert!(m.is_ai(&iri(b.broken)).await.is_err());
}
