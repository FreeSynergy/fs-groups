//! `GroupsRemote`: the groups server behind a socket, as a [`GroupsApi`].
//!
//! Pattern: Proxy — `proxy.rs` is the proxy and its worker thread, `api.rs` the requests,
//! `convert.rs` the conversions from the wire, `model.rs` the types and the trait.

mod api;
mod convert;
mod model;
mod proxy;

pub use model::{ApiError, Group, GroupsApi, Member};
pub use proxy::{GroupsRemote, Limits};
