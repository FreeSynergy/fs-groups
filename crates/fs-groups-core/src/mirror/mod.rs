//! Mirroring our groups into a directory (ADR 0047).
//!
//! Pattern: Facade (declarations) — `Mirror` is the Template method, `SyncReport` its result, `plan`
//! the pure comparison.

mod plan;
mod report;
mod sync;
mod wanted;

pub use plan::{Change, plan};
pub use report::{Divergence, SyncReport};
pub use sync::Mirror;
pub(crate) use sync::hold_still;
pub use wanted::{Scope, Wanted};
