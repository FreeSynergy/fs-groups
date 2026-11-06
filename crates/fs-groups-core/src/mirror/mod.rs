//! Mirroring our groups into a directory (ADR 0047).
//!
//! Pattern: Facade (declarations) — `Mirror` is the Template method, `SyncReport` its result, `plan`
//! the pure comparison.

mod plan;
mod report;
mod sync;

pub use plan::{Change, plan};
pub use report::{Divergence, SyncReport};
pub use sync::Mirror;
