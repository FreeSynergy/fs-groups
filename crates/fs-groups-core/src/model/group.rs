//! The group value and its AS2 `Group` form.
//!
//! Pattern: Value object + Adapter — `as2` / `from_content` map between the typed value and the
//! AS2 JSON that is the authoritative content (kind and parents are the `fs:` extension members).

use fs_store::ObjectId;
use serde_json::{Value, json};

use super::RightFlow;

/// Namespace of the members this program adds to AS2.
pub const NS: &str = "https://freesynergy.net/ns#";

/// A group: an AS2 `Group` with a free-text kind and any number of parents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Id (UUIDv7); the key for everything else.
    pub id: ObjectId,
    /// `https://<instance>/groups/<id>`.
    pub iri: String,
    /// Display name (also the envelope name, ADR 0032).
    pub name: String,
    /// Team, project, department, ... free text, not a fixed list.
    pub kind: String,
    /// Direct parents; a group may have several. Never contains a cycle.
    pub parents: Vec<ObjectId>,
    /// Whether and where this group passes its rights on; `None` when unset.
    pub right_flow: RightFlow,
}

impl Group {
    /// The AS2 `Group` as JSON: the authoritative content of the stored object.
    pub fn as2(&self) -> Value {
        json!({
            "@context": ["https://www.w3.org/ns/activitystreams", {"fs": NS}],
            "type": "Group",
            "id": self.iri,
            "name": self.name,
            "fs:kind": self.kind,
            "fs:parents": self.parents.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "fs:rightFlow": self.right_flow.as_str(),
        })
    }

    /// Read a group back from its AS2 content. `None` if it is not a well-formed group.
    pub fn from_content(id: ObjectId, content: &Value) -> Option<Group> {
        let text = |k: &str| content.get(k)?.as_str().map(str::to_string);
        let parents = content
            .get("fs:parents")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str()?.parse().ok())
            .collect();
        Some(Group {
            id,
            iri: text("id")?,
            name: text("name")?,
            kind: text("fs:kind")?,
            parents,
            // An object written before the setting existed has none: it passes nothing on.
            right_flow: text("fs:rightFlow")
                .and_then(|s| RightFlow::parse(&s))
                .unwrap_or_default(),
        })
    }
}
