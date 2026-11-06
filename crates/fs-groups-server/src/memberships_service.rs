//! `Memberships` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Memberships` call, wire response out.

use std::rc::Rc;
use std::sync::Arc;

use fs_groups_core::Memberships;
use fs_interfaces::groups_capnp::memberships;

use crate::errors::rpc_error;
use crate::wire::{
    read_id, read_member, read_text, write_effective, write_ids, write_membership, write_memberships,
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
        let p = params.get()?;
        let group = read_id(p.get_group())?;
        let member = read_member(p.get_member()?)?;
        let role = read_text(p.get_role())?;
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
        let id = read_id(params.get()?.get_membership())?;
        self.memberships.remove(id).await.map_err(rpc_error)
    }

    async fn direct(
        self: Rc<Self>,
        params: memberships::DirectParams,
        mut results: memberships::DirectResults,
    ) -> Done {
        let group = read_id(params.get()?.get_group())?;
        let found = self.memberships.direct(group).await.map_err(rpc_error)?;
        write_memberships(&found, |n| results.get().init_items(n))
    }

    async fn effective(
        self: Rc<Self>,
        params: memberships::EffectiveParams,
        mut results: memberships::EffectiveResults,
    ) -> Done {
        let group = read_id(params.get()?.get_group())?;
        let found = self.memberships.effective(group).await.map_err(rpc_error)?;
        write_effective(&found, |n| results.get().init_items(n))
    }

    async fn groups_of(
        self: Rc<Self>,
        params: memberships::GroupsOfParams,
        mut results: memberships::GroupsOfResults,
    ) -> Done {
        let p = params.get()?;
        let person = read_text(p.get_person())?;
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
        let p = params.get()?;
        let (group, person) = (read_id(p.get_group())?, read_text(p.get_person())?);
        let member = self
            .memberships
            .is_member(group, person)
            .await
            .map_err(rpc_error)?;
        results.get().set_member(member);
        Ok(())
    }
}
