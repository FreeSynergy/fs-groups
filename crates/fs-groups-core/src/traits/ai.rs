//! The [`AiMarking`] strategy: does the host consider a member an agent?
//!
//! Pattern: Strategy (injected) — this crate does not depend on fs-people. The host wires the answer
//! (for example to `RoleBook::is_ai`). ADR 0056: an agent never becomes admin.

use async_trait::async_trait;

use crate::error::Result;

/// Tells whether a member is marked as an AI agent. Interface status: draft (ADR 0045).
#[async_trait]
pub trait AiMarking: Send + Sync {
    /// `Some(true)`: marked as an agent. `Some(false)`: known, and not one. `None`: no answer is
    /// available. **No answer is treated like "agent"** wherever admin rights are at stake. An error
    /// is passed on; it never counts as "not an agent".
    async fn is_ai(&self, person: &str) -> Result<Option<bool>>;
}
