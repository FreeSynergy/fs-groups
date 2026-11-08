//! `Memberships` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Memberships` call, wire response out.

use std::rc::Rc;
use std::sync::Arc;

use fs_groups_core::Memberships;
use fs_interfaces::groups_capnp::memberships;
use fs_interfaces::rpc::{DEFAULT_PAGE_SIZE, Page};

use crate::errors::rpc_error;
use crate::wire::{
    effective_weight, membership_weight, read_cursor, read_id, read_identifier, read_member,
    request, write_effective, write_ids, write_membership, write_memberships,
};

pub(crate) struct MembershipsImpl {
    pub(crate) memberships: Arc<dyn Memberships>,
}

type Done = Result<(), capnp::Error>;

impl memberships::Server for MembershipsImpl {
    async fn add(
        self: Rc<Self>,
        params: memberships::AddParams,
        mut results: memberships::AddResults,
    ) -> Done {
        let p = request(params.get())?;
        let group = read_id(p.get_group())?;
        let member = read_member(p.get_member())?;
        let role = read_identifier(p.get_role())?;
        let membership = self
            .memberships
            .add(group, member, role)
            .await
            .map_err(rpc_error)?;
        write_membership(results.get().init_membership(), &membership);
        Ok(())
    }

    async fn remove(
        self: Rc<Self>,
        params: memberships::RemoveParams,
        _: memberships::RemoveResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_membership())?;
        self.memberships.remove(id).await.map_err(rpc_error)
    }

    async fn direct(
        self: Rc<Self>,
        params: memberships::DirectParams,
        mut results: memberships::DirectResults,
    ) -> Done {
        let p = request(params.get())?;
        let group = read_id(p.get_group())?;
        let found = self.memberships.direct(group).await.map_err(rpc_error)?;
        let cursor = read_cursor(p.get_cursor())?;
        let page = Page::of_weighted(found, cursor, DEFAULT_PAGE_SIZE, membership_weight)?;
        write_memberships(&page.items, |n| results.get().init_items(n))?;
        results.get().init_next().set_opaque(&page.next);
        Ok(())
    }

    async fn effective(
        self: Rc<Self>,
        params: memberships::EffectiveParams,
        mut results: memberships::EffectiveResults,
    ) -> Done {
        let p = request(params.get())?;
        let group = read_id(p.get_group())?;
        let found = self.memberships.effective(group).await.map_err(rpc_error)?;
        let cursor = read_cursor(p.get_cursor())?;
        let page = Page::of_weighted(found, cursor, DEFAULT_PAGE_SIZE, effective_weight)?;
        write_effective(&page.items, |n| results.get().init_items(n))?;
        results.get().init_next().set_opaque(&page.next);
        Ok(())
    }

    async fn groups_of(
        self: Rc<Self>,
        params: memberships::GroupsOfParams,
        mut results: memberships::GroupsOfResults,
    ) -> Done {
        let p = request(params.get())?;
        let person = read_identifier(p.get_person())?;
        let ids = self
            .memberships
            .groups_of(person)
            .await
            .map_err(rpc_error)?;
        write_ids(&ids, |n| results.get().init_ids(n))
    }

    async fn is_member(
        self: Rc<Self>,
        params: memberships::IsMemberParams,
        mut results: memberships::IsMemberResults,
    ) -> Done {
        let p = request(params.get())?;
        let (group, person) = (read_id(p.get_group())?, read_identifier(p.get_person())?);
        let member = self
            .memberships
            .is_member(group, person)
            .await
            .map_err(rpc_error)?;
        results.get().set_member(member);
        Ok(())
    }
}
