//! The [`GroupDirectory`] provider interface: a directory that mirrors groups.
//!
//! Pattern: Strategy + Adapter (interface) — fs-groups says what should be; an adapter (Kanidm, ...)
//! brings its directory to that state. The Rust form of `GroupDirectory` in `groups.capnp`.

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;

use crate::error::Result;
use crate::unsupported::not_possible;

/// A group as a directory should hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryGroup {
    /// The group id of fs-groups.
    pub id: ObjectId,
    /// Display name.
    pub name: String,
    /// The people listed in this group, as the IRIs fs-groups holds: its direct people and, for a
    /// child that is not nested (option B, ADR 0056), the people of that child's whole subtree. A
    /// group that carries an administrative right never lists an agent or an unknown person.
    pub members: Vec<String>,
    /// The child groups nested in this one: only where both pass `Up` and nothing forbids it.
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
    /// Who this directory is: one value per directory, stable across runs and different for two
    /// directories of one provider (for example the URL of the Kanidm instance). The mirror ledger
    /// is kept under it. The default is the provider name, which two directories of one provider
    /// **share**: an adapter that can run twice must override it (`fs-identity` for Kanidm).
    fn directory_id(&self) -> String {
        self.capabilities().provider().to_string()
    }
    /// `directory.read`: one group by fs-groups id, if the directory holds it.
    async fn read(&self, id: ObjectId) -> Result<Option<DirectoryGroup>>;
    /// `directory.read`: every group the directory holds that fs-groups put there.
    async fn list(&self) -> Result<Vec<DirectoryGroup>>;

    /// `directory.apply`: bring the group in the directory to this state. Idempotent. A person the
    /// directory cannot map to an account must not stop the rest: apply everything else (also the
    /// removals) and answer [`Error::Unmapped`](crate::Error::Unmapped) with the people left out.
    async fn apply(&self, _group: &DirectoryGroup) -> Result<()> {
        not_possible("directory.apply")
    }
    /// `directory.apply`: remove the group; removing an absent one is not an error.
    async fn remove(&self, _id: ObjectId) -> Result<()> {
        not_possible("directory.apply")
    }
}
