//! The contract as Rust traits: groups, memberships, invitations, delegations, feed, directory.
//!
//! Pattern: Facade (declarations) — callers depend on these traits, never on `GroupRepo`.

mod delegations;
mod directory;
mod feed;
mod follows;
mod groups;
mod invitations;
mod memberships;
mod walk;

pub use delegations::Delegations;
pub use directory::{DirectoryGroup, GroupDirectory};
pub use feed::{GroupChange, GroupFeed, GroupListener, Registry, Subscription};
pub use follows::Follows;
pub use groups::Groups;
pub use invitations::Invitations;
pub use memberships::Memberships;
