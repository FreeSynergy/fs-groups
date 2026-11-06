//! The [`GroupDirectory`] provider interface: a directory that mirrors groups.
//!
//! Pattern: Strategy + Adapter (interface) — fs-groups says what should be; an adapter (Kanidm, ...)
//! brings its directory to that state. The Rust form of `GroupDirectory` in `groups.capnp`.

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

use crate::error::Result;
use crate::unsupported::not_possible;

/// A group as a directory should hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryGroup {
    /// The group id of fs-groups.
    pub id: ObjectId,
    /// Display name.
    pub name: String,
    /// Direct people, as the IRIs fs-groups holds.
    pub members: Vec<String>,
    /// Direct groups (children).
    pub nested: Vec<ObjectId>,
}

impl DirectoryGroup {
    /// The same group with its lists sorted, so two of them compare without caring for order.
    #[must_use]
    pub fn normalised(mut self) -> Self {
        self.members.sort();
        self.members.dedup();
        self.nested.sort();
        self.nested.dedup();
        self
    }
}

/// A directory that mirrors groups (ADR 0047). Interface status: draft (ADR 0045).
#[async_trait]
pub trait GroupDirectory: Send + Sync {
    /// What this directory can do, per capability id.
    fn capabilities(&self) -> Capabilities;
    /// `directory.read`: one group by fs-groups id, if the directory holds it.
    async fn read(&self, id: ObjectId) -> Result<Option<DirectoryGroup>>;
    /// `directory.read`: every group the directory holds that fs-groups put there.
    async fn list(&self) -> Result<Vec<DirectoryGroup>>;

    /// `directory.apply`: bring the group in the directory to this state. Idempotent.
    async fn apply(&self, _group: &DirectoryGroup) -> Result<()> {
        not_possible("directory.apply")
    }
    /// `directory.apply`: remove the group; removing an absent one is not an error.
    async fn remove(&self, _id: ObjectId) -> Result<()> {
        not_possible("directory.apply")
    }
}
