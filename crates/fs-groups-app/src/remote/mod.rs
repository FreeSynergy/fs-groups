//! The connection to the groups server: a proxy with a worker thread, and the errors of a call.
//!
//! Pattern: Proxy (`proxy.rs`) + Adapter (`error.rs`).

mod error;
mod proxy;

pub use error::RunError;
pub use proxy::{Clients, GroupsRemote};
