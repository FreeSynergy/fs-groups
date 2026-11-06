//! The nesting methods of `Items`: ask, approve, decline, list requests, end a link.
//!
//! Pattern: Adapter — wire request in, `Groups` call, wire response out; split from `items_service` to
//! keep one concept per file. Who consents and the cycle rule stay in the core. The contract's
//! `approveParent` names nobody: the core asks about its configured actor, a string nothing
//! authenticates (`DESIGN.md`).

use fs_groups_core::Groups;
use fs_interfaces::groups_capnp::items;

use crate::errors::rpc_error;
use crate::wire::{read_id, write_group, write_parent_request, write_parent_requests};

/// `addParent @6`, which the contract marks superseded: its capability id and the reason it answers.
pub(crate) const ADD_PARENT: (&str, &str) = (
    "group.addParent",
    "superseded by requestParent: a parent has to consent to a link",
);

type Done = Result<(), capnp::Error>;

pub(crate) async fn remove_parent(
    groups: &dyn Groups,
    params: items::RemoveParentParams,
    mut results: items::RemoveParentResults,
) -> Done {
    let p = params.get()?;
    let (id, parent) = (read_id(p.get_id())?, read_id(p.get_parent())?);
    let group = groups.remove_parent(id, parent).await.map_err(rpc_error)?;
    write_group(results.get().init_group(), &group)
}

pub(crate) async fn request_parent(
    groups: &dyn Groups,
    params: items::RequestParentParams,
    mut results: items::RequestParentResults,
) -> Done {
    let p = params.get()?;
    let (child, parent) = (read_id(p.get_child())?, read_id(p.get_parent())?);
    let request = groups
        .request_parent(child, parent)
        .await
        .map_err(rpc_error)?;
    write_parent_request(results.get().init_request(), &request);
    Ok(())
}

pub(crate) async fn approve_parent(
    groups: &dyn Groups,
    params: items::ApproveParentParams,
    mut results: items::ApproveParentResults,
) -> Done {
    let request = read_id(params.get()?.get_request())?;
    let group = groups.approve_parent(request).await.map_err(rpc_error)?;
    write_group(results.get().init_group(), &group)
}

pub(crate) async fn decline_parent(
    groups: &dyn Groups,
    params: items::DeclineParentParams,
) -> Done {
    let request = read_id(params.get()?.get_request())?;
    groups.decline_parent(request).await.map_err(rpc_error)
}

pub(crate) async fn parent_requests(
    groups: &dyn Groups,
    params: items::ParentRequestsParams,
    mut results: items::ParentRequestsResults,
) -> Done {
    let parent = read_id(params.get()?.get_parent())?;
    let found = groups.parent_requests(parent).await.map_err(rpc_error)?;
    write_parent_requests(&found, |n| results.get().init_items(n))
}
