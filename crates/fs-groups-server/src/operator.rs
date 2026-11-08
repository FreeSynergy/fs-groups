//! The marking the server wires: the operator is a person, nobody else is known.
//!
//! Pattern: Strategy (the injected `AiMarking`) — whoever starts the server and names `--actor` takes
//! the responsibility for that identity being a human; every other identity gets no answer, which
//! the core treats like an agent where admin rights are at stake. Provisional: when a people provider
//! is reachable, `fs-groups-people` replaces this.

use async_trait::async_trait;
use fs_groups_core::{AiMarking, Result};

/// Says `Some(false)` for the configured actor and nothing for anybody else.
pub struct OperatorIsHuman(String);

impl OperatorIsHuman {
    /// The operator; scheme and host are folded to lower case and the text is trimmed, as the core
    /// does before it asks, so `HTTPS://Host/x ` and `https://host/x` are one identity.
    #[must_use]
    pub fn new(actor: &str) -> Self {
        Self(fold(actor))
    }
}

fn fold(s: &str) -> String {
    let s = s.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return s.to_string();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let (user, host) = authority
        .rfind('@')
        .map_or(("", authority), |i| authority.split_at(i + 1));
    format!(
        "{}://{user}{}{tail}",
        scheme.to_lowercase(),
        host.to_lowercase()
    )
}

#[async_trait]
impl AiMarking for OperatorIsHuman {
    async fn is_ai(&self, person: &str) -> Result<Option<bool>> {
        Ok((fold(person) == self.0).then_some(false))
    }
}
