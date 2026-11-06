//! Envelope and stored-object building for every object kind of this program.
//!
//! Pattern: Adapter — maps the typed values onto AS2 envelopes plus index fields that `fs-store`
//! understands (ADR 0020). The envelope `name` is derived, never typed separately (ADR 0032).

use fs_store::{Envelope, StoredObject};
use serde_json::{Value, json};

use super::consts::{AS2_TYPE, CONTEXT, GROUP_TYPE};
use super::handle::GroupRepo;
use crate::model::Group;

/// The JSON-LD context of the AS2 content written here.
pub(super) fn context() -> Value {
    json!(["https://www.w3.org/ns/activitystreams", {"fs": crate::model::NS}])
}

/// The store refuses empty index values, so an empty role or kind is simply not indexed.
pub(super) fn with_nonempty(o: StoredObject, key: &str, value: &str) -> StoredObject {
    if value.is_empty() {
        o
    } else {
        o.with_index(key, value)
    }
}

impl GroupRepo {
    /// A fresh envelope of `object_type` with this program's context, actor and IRI.
    pub(super) fn fresh_envelope(&self, object_type: &str, name: &str) -> Envelope {
        let mut e = Envelope::new(object_type, name);
        e.iri = Some(self.iri(e.id));
        e.context = Some(CONTEXT.into());
        e.attributed_to = Some(self.actor.clone());
        e.to = vec![self.actor.clone()];
        e
    }

    /// The stored object of a group, keeping its envelope (id, published); the name is derived.
    pub(super) fn group_object(g: &Group) -> StoredObject {
        let mut env = g.envelope.clone();
        env.name.clone_from(&g.name);
        let mut o = with_nonempty(StoredObject::new(env, AS2_TYPE, g.as2()), "kind", &g.kind);
        for p in &g.parents {
            o = o.with_index("parent", p.to_string());
        }
        o
    }

    /// A new group value with a fresh id and envelope.
    pub(super) fn new_group(&self, name: &str, kind: &str) -> Group {
        let envelope = self.fresh_envelope(GROUP_TYPE, name);
        let id = envelope.id;
        Group {
            id,
            iri: self.iri(id),
            name: name.into(),
            kind: kind.into(),
            parents: vec![],
            right_flow: crate::model::RightFlow::default(),
            envelope,
        }
    }
}
