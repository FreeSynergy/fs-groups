//! The types the window sees: `Group`, `Member`, `ApiError` and the `GroupsApi` trait.
//!
//! Pattern: Proxy — `GroupsApi` is the trait first; the adapter knows only it.

/// A group as the window needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The id.
    pub id: String,
    /// The name.
    pub name: String,
    /// What kind of group: free text.
    pub kind: String,
    /// The ids of the groups it belongs to.
    pub parents: Vec<String>,
}

/// Who is in a group, directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// A person (an IRI) or, for a group, its id.
    pub who: String,
    /// Whether `who` is a group.
    pub is_group: bool,
    /// The role id; empty for a plain member.
    pub role: String,
}

/// Why a call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// There is no such group.
    NotFound(String),
    /// The server cannot do this (the provider rule).
    Unsupported(String),
    /// The server refused or failed; the text is its own.
    Failed(String),
    /// The connection to the server is lost (it went away); the next call connects anew.
    Lost(String),
}

/// What the window asks of the groups program.
pub trait GroupsApi: Send + Sync {
    /// All groups, in the server's order.
    ///
    /// # Errors
    /// [`ApiError`].
    fn list(&self) -> Result<Vec<Group>, ApiError>;
    /// One group, `None` if there is none.
    ///
    /// # Errors
    /// [`ApiError`].
    fn get(&self, id: &str) -> Result<Option<Group>, ApiError>;
    /// A new group.
    ///
    /// # Errors
    /// [`ApiError`].
    fn create(&self, name: &str, kind: &str) -> Result<Group, ApiError>;
    /// Change the name.
    ///
    /// # Errors
    /// [`ApiError`].
    fn rename(&self, id: &str, name: &str) -> Result<Group, ApiError>;
    /// Change the kind.
    ///
    /// # Errors
    /// [`ApiError`].
    fn set_kind(&self, id: &str, kind: &str) -> Result<Group, ApiError>;
    /// Delete the group, with its memberships.
    ///
    /// # Errors
    /// [`ApiError`].
    fn delete(&self, id: &str) -> Result<(), ApiError>;
    /// The direct members.
    ///
    /// # Errors
    /// [`ApiError`].
    fn members(&self, id: &str) -> Result<Vec<Member>, ApiError>;
}
