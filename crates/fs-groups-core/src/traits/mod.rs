//! The contract as Rust traits: groups, memberships, invitations, delegations, directory.
//!
//! Pattern: Facade (declarations) — callers depend on these traits, never on `GroupRepo`.

mod ai;
mod delegations;
mod directory;
mod groups;
mod invitations;
mod memberships;
mod walk;

pub use ai::AiMarking;
pub use delegations::Delegations;
pub use directory::{DirectoryGroup, GroupDirectory};
pub use groups::Groups;
pub use invitations::Invitations;
pub use memberships::Memberships;
