//! `PeopleMarking`: the bridge from the people role book to `fs_groups_core::AiMarking`.
//!
//! Pattern: Adapter — `fs-groups-core` does not depend on `fs-people`; this crate does, and turns the
//! role book into the answer the groups ask for (ADR 0054, 0056). The decision "is this role the AI
//! role" is `fs_people_core::is_ai_role`, never a literal, so the short id `ai` and the full id
//! `https://freesynergy.net/ns/roles/ai` both count.
//!
//! **Answers follow the trait's rule: `Some(false)` only for a person known to be a human.** The role
//! book cannot tell a person without roles from an unknown one, so a person with no role at all, an
//! identity that is not `…/<id>`, and an id the book refuses are `None`, and groups treat that like an
//! agent wherever admin rights are at stake. An error of the book is passed on, never taken for a human.

use std::sync::Arc;

use async_trait::async_trait;
use fs_groups_core::{AiMarking, Error, Result};
use fs_objects::ObjectId;
use fs_people_core::{RoleBook, is_ai_role};

/// `AiMarking` over a role book.
pub struct PeopleMarking {
    book: Arc<dyn RoleBook>,
}

impl PeopleMarking {
    /// Mark by the roles `book` holds.
    pub fn new(book: Arc<dyn RoleBook>) -> Self {
        Self { book }
    }
}

/// The person id in an IRI `https://<host>/people/<id>`: the last path segment.
fn person_id(iri: &str) -> Option<ObjectId> {
    let path = iri.split(['?', '#']).next()?.trim_end_matches('/');
    path.rsplit('/').next()?.parse().ok()
}

#[async_trait]
impl AiMarking for PeopleMarking {
    async fn is_ai(&self, person: &str) -> Result<Option<bool>> {
        let Some(id) = person_id(person) else {
            return Ok(None);
        };
        let roles = self
            .book
            .roles_of(id)
            .await
            .map_err(|e| Error::AiStatusUnknown(format!("role book: {e}")))?;
        if roles.is_empty() {
            return Ok(None);
        }
        Ok(Some(roles.iter().any(|a| is_ai_role(&a.role))))
    }
}
