//! The marking the server wires: the operator is a person, nobody else is known.
//!
//! Pattern: Strategy (the injected `AiMarking`) — whoever starts the server and names `--actor` takes
//! the responsibility for that identity being a human; every other identity gets no answer, which
//! the core treats like an agent where admin rights are at stake. Provisional: when a people provider
//! is reachable, `fs-groups-people` replaces this.

use async_trait::async_trait;
use fs_groups_core::{AiMarking, Result};

/// Says `Some(false)` for the configured actor and nothing for anybody else.
pub struct OperatorIsHuman(pub String);

#[async_trait]
impl AiMarking for OperatorIsHuman {
    async fn is_ai(&self, person: &str) -> Result<Option<bool>> {
        Ok((person == self.0).then_some(false))
    }
}
