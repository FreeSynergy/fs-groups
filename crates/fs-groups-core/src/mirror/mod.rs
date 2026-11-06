//! Mirroring our groups into a directory (ADR 0047).
//!
//! Pattern: Facade (declarations) — `Mirror` is the Template method, `SyncReport` its result, `plan`
//! the pure comparison.

mod enforce;
mod plan;
mod report;
mod steps;
mod sync;
mod wanted;

pub use enforce::{Enforced, closure as enforce_closure, enforce};
pub use plan::{Change, plan};
pub use report::{Divergence, SyncReport};
pub use sync::Mirror;
pub use wanted::{AdminRule, Bar, Scope, Verdict, Wanted};
