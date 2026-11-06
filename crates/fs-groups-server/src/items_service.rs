//! `Items` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Groups` call, wire response out. What a provider cannot do
//! arrives as its explicit `Unsupported` and leaves as a typed problem.

use std::rc::Rc;
use std::sync::Arc;

use fs_groups_core::Groups;
use fs_interfaces::Unsupported;
use fs_interfaces::groups_capnp::items;
use fs_interfaces::rpc::{run_to_end, unsupported};

use crate::errors::rpc_error;
use crate::list_all::all_groups;
use crate::nesting_service::{self as nesting, ADD_PARENT};
use crate::wire::{read_flow, read_id, read_text, request, write_group, write_ids, write_page};

pub(crate) struct ItemsImpl {
    pub(crate) groups: Arc<dyn Groups>,
}

type Done = Result<(), capnp::Error>;

impl items::Server for ItemsImpl {
    async fn create(
        self: Rc<Self>,
        params: items::CreateParams,
        mut results: items::CreateResults,
    ) -> Done {
        let p = request(params.get())?;
        let (name, kind) = (read_text(p.get_name())?, read_text(p.get_kind())?);
        let group = self.groups.create(name, kind).await.map_err(rpc_error)?;
        write_group(results.get().init_group(), &group)
    }

    async fn rename(
        self: Rc<Self>,
        params: items::RenameParams,
        mut results: items::RenameResults,
    ) -> Done {
        let p = request(params.get())?;
        let (id, name) = (read_id(p.get_id())?, read_text(p.get_name())?);
        let group = self.groups.rename(id, name).await.map_err(rpc_error)?;
        write_group(results.get().init_group(), &group)
    }

    async fn set_kind(
        self: Rc<Self>,
        params: items::SetKindParams,
        mut results: items::SetKindResults,
    ) -> Done {
        let p = request(params.get())?;
        let (id, kind) = (read_id(p.get_id())?, read_text(p.get_kind())?);
        let group = self.groups.set_kind(id, kind).await.map_err(rpc_error)?;
        write_group(results.get().init_group(), &group)
    }

    async fn set_right_flow(
        self: Rc<Self>,
        params: items::SetRightFlowParams,
        mut results: items::SetRightFlowResults,
    ) -> Done {
        let p = request(params.get())?;
        let (id, flow) = (read_id(p.get_id())?, read_flow(p.get_flow())?);
        let group = self
            .groups
            .set_right_flow(id, flow)
            .await
            .map_err(rpc_error)?;
        write_group(results.get().init_group(), &group)
    }

    async fn delete(self: Rc<Self>, params: items::DeleteParams, _: items::DeleteResults) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        // A cascade of many steps: it finishes even if the client goes away (`run_to_end`).
        let groups = self.groups.clone();
        run_to_end(async move { groups.delete(id).await })
            .await?
            .map_err(rpc_error)
    }

    async fn get(self: Rc<Self>, params: items::GetParams, mut results: items::GetResults) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        let found = self.groups.get(id).await.map_err(rpc_error)?;
        let mut r = results.get();
        r.set_found(found.is_some());
        match found {
            Some(group) => write_group(r.init_group(), &group),
            None => Ok(()),
        }
    }

    async fn list(
        self: Rc<Self>,
        params: items::ListParams,
        mut results: items::ListResults,
    ) -> Done {
        let all = all_groups(self.groups.as_ref()).await?;
        write_page(
            results.get().init_page(),
            all,
            request(params.get())?.get_cursor(),
        )
    }

    /// Superseded in the contract: a parent now has to consent.
    async fn add_parent(
        self: Rc<Self>,
        _: items::AddParentParams,
        _: items::AddParentResults,
    ) -> Done {
        Err(unsupported(&Unsupported::new(ADD_PARENT.0, ADD_PARENT.1)))
    }

    async fn remove_parent(
        self: Rc<Self>,
        params: items::RemoveParentParams,
        results: items::RemoveParentResults,
    ) -> Done {
        nesting::remove_parent(self.groups.as_ref(), params, results).await
    }

    async fn request_parent(
        self: Rc<Self>,
        params: items::RequestParentParams,
        results: items::RequestParentResults,
    ) -> Done {
        nesting::request_parent(self.groups.as_ref(), params, results).await
    }

    async fn approve_parent(
        self: Rc<Self>,
        params: items::ApproveParentParams,
        results: items::ApproveParentResults,
    ) -> Done {
        nesting::approve_parent(self.groups.clone(), params, results).await
    }

    async fn decline_parent(
        self: Rc<Self>,
        params: items::DeclineParentParams,
        _: items::DeclineParentResults,
    ) -> Done {
        nesting::decline_parent(self.groups.as_ref(), params).await
    }

    async fn parent_requests(
        self: Rc<Self>,
        params: items::ParentRequestsParams,
        results: items::ParentRequestsResults,
    ) -> Done {
        nesting::parent_requests(self.groups.as_ref(), params, results).await
    }

    async fn parents(
        self: Rc<Self>,
        params: items::ParentsParams,
        mut results: items::ParentsResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        let ids = self.groups.parents(id).await.map_err(rpc_error)?;
        write_ids(&ids, |n| results.get().init_ids(n))
    }

    async fn children(
        self: Rc<Self>,
        params: items::ChildrenParams,
        mut results: items::ChildrenResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        let ids = self.groups.children(id).await.map_err(rpc_error)?;
        write_ids(&ids, |n| results.get().init_ids(n))
    }

    async fn ancestors(
        self: Rc<Self>,
        params: items::AncestorsParams,
        mut results: items::AncestorsResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        let ids = self.groups.ancestors(id).await.map_err(rpc_error)?;
        write_ids(&ids, |n| results.get().init_ids(n))
    }

    async fn descendants(
        self: Rc<Self>,
        params: items::DescendantsParams,
        mut results: items::DescendantsResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        let ids = self.groups.descendants(id).await.map_err(rpc_error)?;
        write_ids(&ids, |n| results.get().init_ids(n))
    }
}
