//! Every group of a provider, whatever its page size.
//!
//! Pattern: Iterator — walks the provider's own pages through its cursor; the wire page is cut by
//! weight afterwards, which a provider's page does not know about.

use fs_groups_core::{Group, Groups};

use crate::errors::rpc_error;

/// The whole list through the provider's paging; the wire page is cut by weight afterwards.
pub(crate) async fn all_groups(groups: &dyn Groups) -> Result<Vec<Group>, capnp::Error> {
    let mut all = Vec::new();
    let mut cursor = None;
    loop {
        let page = groups.list(cursor.as_ref()).await.map_err(rpc_error)?;
        all.extend(page.items);
        match page.next {
            Some(next) => cursor = Some(next),
            None => return Ok(all),
        }
    }
}
