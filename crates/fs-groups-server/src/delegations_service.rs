//! `Delegations` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Delegations` call, wire response out. `mayDo` only answers the
//! question, as in the core; nothing here asks it (`DESIGN.md`, open points).

use std::rc::Rc;
use std::sync::Arc;

use fs_groups_core::Delegations;
use fs_interfaces::groups_capnp::delegations;

use crate::errors::rpc_error;
use crate::wire::{read_id, read_member, read_right, read_text, write_delegation, write_delegations};

pub(crate) struct DelegationsImpl {
    pub(crate) delegations: Arc<dyn Delegations>,
}

type Done = Result<(), capnp::Error>;

impl delegations::Server for DelegationsImpl {
    async fn grant(
        self: Rc<Self>,
        params: delegations::GrantParams,
        mut results: delegations::GrantResults,
    ) -> Done {
        let p = params.get()?;
        let group = read_id(p.get_group())?;
        let holder = read_member(p.get_holder()?)?;
        let right = read_right(p.get_right())?;
        let delegation = self
            .delegations
            .grant(group, holder, right)
            .await
            .map_err(rpc_error)?;
        write_delegation(results.get().init_delegation(), &delegation);
        Ok(())
    }

    async fn revoke(
        self: Rc<Self>,
        params: delegations::RevokeParams,
        _: delegations::RevokeResults,
    ) -> Done {
        let id = read_id(params.get()?.get_id())?;
        self.delegations.revoke(id).await.map_err(rpc_error)
    }

    async fn list(
        self: Rc<Self>,
        params: delegations::ListParams,
        mut results: delegations::ListResults,
    ) -> Done {
        let group = read_id(params.get()?.get_group())?;
        let found = self.delegations.list(group).await.map_err(rpc_error)?;
        write_delegations(&found, |n| results.get().init_items(n))
    }

    async fn may_do(
        self: Rc<Self>,
        params: delegations::MayDoParams,
        mut results: delegations::MayDoResults,
    ) -> Done {
        let p = params.get()?;
        let group = read_id(p.get_group())?;
        let person = read_text(p.get_person())?;
        let right = read_right(p.get_right())?;
        let allowed = self
            .delegations
            .may_do(group, person, right)
            .await
            .map_err(rpc_error)?;
        results.get().set_allowed(allowed);
        Ok(())
    }
}
