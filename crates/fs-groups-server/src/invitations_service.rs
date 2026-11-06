//! `Invitations` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Invitations` call, wire response out. The core takes the time
//! as an argument; this adapter is where the clock is read.

use std::rc::Rc;
use std::sync::Arc;

use chrono::Utc;
use fs_groups_core::{Error, Invitations};
use fs_interfaces::groups_capnp::invitations;

use crate::errors::rpc_error;
use crate::wire::{
    read_expires, read_id, read_text, write_invitation, write_invitations, write_membership,
};

pub(crate) struct InvitationsImpl {
    pub(crate) invitations: Arc<dyn Invitations>,
}

type Done = Result<(), capnp::Error>;

impl invitations::Server for InvitationsImpl {
    async fn invite(
        self: Rc<Self>,
        params: invitations::InviteParams,
        mut results: invitations::InviteResults,
    ) -> Done {
        let p = params.get()?;
        let group = read_id(p.get_group())?;
        let (invitee, role) = (read_text(p.get_invitee())?, read_text(p.get_role())?);
        let expires = if p.has_expires() {
            Some(read_expires(p.get_expires()?)?)
        } else {
            None
        };
        let invitation = self
            .invitations
            .invite(group, invitee, role, expires)
            .await
            .map_err(rpc_error)?;
        write_invitation(results.get().init_invitation(), &invitation);
        Ok(())
    }

    async fn accept(
        self: Rc<Self>,
        params: invitations::AcceptParams,
        mut results: invitations::AcceptResults,
    ) -> Done {
        let p = params.get()?;
        let (id, person) = (read_id(p.get_id())?, read_text(p.get_person())?);
        let membership = self
            .invitations
            .accept(id, person, Utc::now())
            .await
            .map_err(rpc_error)?;
        write_membership(results.get().init_membership(), &membership);
        Ok(())
    }

    /// Passed on as asked: the contract has the caller vouch for the address, and nothing here checks it.
    async fn accept_proved(
        self: Rc<Self>,
        params: invitations::AcceptProvedParams,
        mut results: invitations::AcceptProvedResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_id(p.get_id())?;
        let (person, proved) = (
            read_text(p.get_person())?,
            read_text(p.get_proved_address())?,
        );
        let membership = self
            .invitations
            .accept_proved(id, person, proved, Utc::now())
            .await
            .map_err(rpc_error)?;
        write_membership(results.get().init_membership(), &membership);
        Ok(())
    }

    async fn decline(
        self: Rc<Self>,
        params: invitations::DeclineParams,
        _: invitations::DeclineResults,
    ) -> Done {
        let id = read_id(params.get()?.get_id())?;
        self.invitations.decline(id).await.map_err(rpc_error)
    }

    async fn revoke(
        self: Rc<Self>,
        params: invitations::RevokeParams,
        _: invitations::RevokeResults,
    ) -> Done {
        let id = read_id(params.get()?.get_id())?;
        self.invitations.revoke(id).await.map_err(rpc_error)
    }

    /// The contract's `expired` has to be true when read, so what ran out is marked first. A provider
    /// without the sweep still answers its list.
    async fn list(
        self: Rc<Self>,
        params: invitations::ListParams,
        mut results: invitations::ListResults,
    ) -> Done {
        let group = read_id(params.get()?.get_group())?;
        match self.invitations.expire_due(Utc::now()).await {
            Ok(_) | Err(Error::Unsupported(_)) => {}
            Err(e) => return Err(rpc_error(e)),
        }
        let found = self.invitations.list(group).await.map_err(rpc_error)?;
        write_invitations(&found, |n| results.get().init_items(n))
    }
}
