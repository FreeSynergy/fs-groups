//! Admin rules of ADR 0056: an agent never becomes admin.
//!
//! Pattern: Specification — one place that says what counts as admin (`Right::Manage`, plus the
//! configured role ids) and asks the injected [`AiMarking`](crate::traits::AiMarking). **No answer
//! means refuse.** Like the rest, this is only an answer: nothing verifies who calls.

use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::text;

impl GroupRepo {
    /// Whether `role` counts as admin: its folded form is one of the configured roles.
    pub(super) fn is_admin_role(&self, role: &str) -> bool {
        self.admin_roles.contains(&text::fold(role))
    }

    /// The host's answer for `person`, asked in canonical spelling. `None`: no answer, or no checker.
    pub(super) async fn ai_answer(&self, person: &str) -> Result<Option<bool>> {
        match &self.ai {
            None => Ok(None),
            Some(ai) => ai.is_ai(&text::canonical_person(person)).await,
        }
    }

    /// Whether `person` is known to be a human. `false` for an agent, for no answer, and when no
    /// checker is wired. A failing checker is an error.
    pub(super) async fn is_known_human(&self, person: &str) -> Result<bool> {
        Ok(self.ai_answer(person).await? == Some(false))
    }

    /// Refuse an admin right for `person` unless they are known to be a human.
    pub(super) async fn require_human(&self, person: &str, what: &str) -> Result<()> {
        match self.ai_answer(person).await? {
            Some(false) => Ok(()),
            Some(true) => Err(Error::AgentNotAdmin(format!(
                "{person} is marked as an agent ({what})"
            ))),
            None => Err(Error::AiStatusUnknown(person.to_string())),
        }
    }
}
