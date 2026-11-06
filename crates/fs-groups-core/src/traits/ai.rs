//! The [`AiMarking`] strategy: does the host consider a member an agent?
//!
//! Pattern: Strategy (injected) — this crate does not depend on fs-people. The host wires the answer
//! (for example to `RoleBook::is_ai`). ADR 0056: an agent never becomes admin.

use async_trait::async_trait;

use crate::error::Result;

/// Tells whether a member is marked as an AI agent. Interface status: draft (ADR 0045).
///
/// **`Some(false)` means "known to be a human", not "not marked".** An implementation must answer
/// `None` for an identity it does not know. A thin wrapper that answers `Some(false)` for anything
/// without roles (fs-people's `RoleBook::is_ai` does that for an id with no roles) turns unknown into
/// human, and then any spelling the book does not know gets admin rights. The adapter shape:
///
/// ```text
/// async fn is_ai(&self, person: &str) -> Result<Option<bool>> {
///     if self.role_book.knows(person) { Ok(Some(self.role_book.is_ai(person))) } else { Ok(None) }
/// }
/// ```
///
/// This crate asks in canonical spelling (trimmed, scheme and host in lower case, no trailing slash;
/// the path keeps its case) and refuses control and invisible characters before it asks.
#[async_trait]
pub trait AiMarking: Send + Sync {
    /// `Some(true)`: marked as an agent. `Some(false)`: known, and a human. `None`: no answer is
    /// available. **No answer is treated like "agent"** wherever admin rights are at stake. An error
    /// is passed on; it never counts as "human".
    async fn is_ai(&self, person: &str) -> Result<Option<bool>>;
}
