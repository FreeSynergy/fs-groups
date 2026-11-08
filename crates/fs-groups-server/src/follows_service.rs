//! `Follows` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Follows` call, wire response out. **Nothing authenticates
//! `sender`**: on this transport the client names it, so `receive` and `withdraw` are only as safe as
//! "same system user" (`DESIGN.md`); the core checks that the offer's source lies at `sender`.

use std::rc::Rc;
use std::sync::Arc;

use fs_groups_core::Follows;
use fs_interfaces::groups_capnp::follows;

use crate::errors::rpc_error;
use crate::wire::{
    read_id, read_identifier, read_offer, read_rights, request, write_deliveries, write_delivery,
    write_offer, write_source, write_sources,
};

pub(crate) struct FollowsImpl {
    pub(crate) follows: Arc<dyn Follows>,
}

type Done = Result<(), capnp::Error>;

impl follows::Server for FollowsImpl {
    async fn deliver(
        self: Rc<Self>,
        params: follows::DeliverParams,
        mut results: follows::DeliverResults,
    ) -> Done {
        let p = request(params.get())?;
        let group = read_id(p.get_group())?;
        let recipient = read_identifier(p.get_recipient())?;
        let rights = read_rights(p.get_rights())?;
        let (delivery, offer) = self
            .follows
            .deliver(group, recipient, &rights, p.get_pass_on())
            .await
            .map_err(rpc_error)?;
        let mut out = results.get();
        write_delivery(out.reborrow().init_delivery(), &delivery);
        write_offer(out.init_offer(), &offer);
        Ok(())
    }

    async fn pass_on(
        self: Rc<Self>,
        params: follows::PassOnParams,
        mut results: follows::PassOnResults,
    ) -> Done {
        let p = request(params.get())?;
        let source = read_id(p.get_source())?;
        let recipient = read_identifier(p.get_recipient())?;
        let rights = read_rights(p.get_rights())?;
        let (delivery, offer) = self
            .follows
            .pass_on(source, recipient, &rights)
            .await
            .map_err(rpc_error)?;
        let mut out = results.get();
        write_delivery(out.reborrow().init_delivery(), &delivery);
        write_offer(out.init_offer(), &offer);
        Ok(())
    }

    async fn revoke(
        self: Rc<Self>,
        params: follows::RevokeParams,
        _: follows::RevokeResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_delivery())?;
        self.follows.revoke_delivery(id).await.map_err(rpc_error)
    }

    async fn deliveries(
        self: Rc<Self>,
        params: follows::DeliveriesParams,
        mut results: follows::DeliveriesResults,
    ) -> Done {
        let group = read_id(request(params.get())?.get_group())?;
        let found = self.follows.deliveries(group).await.map_err(rpc_error)?;
        write_deliveries(&found, |n| results.get().init_items(n))
    }

    async fn receive(
        self: Rc<Self>,
        params: follows::ReceiveParams,
        mut results: follows::ReceiveResults,
    ) -> Done {
        let p = request(params.get())?;
        let sender = read_identifier(p.get_sender())?;
        let offer = read_offer(p.get_offer())?;
        let source = self
            .follows
            .receive(sender, &offer)
            .await
            .map_err(rpc_error)?;
        write_source(results.get().init_source(), &source);
        Ok(())
    }

    async fn withdraw(
        self: Rc<Self>,
        params: follows::WithdrawParams,
        mut results: follows::WithdrawResults,
    ) -> Done {
        let p = request(params.get())?;
        let sender = read_identifier(p.get_sender())?;
        let source = read_identifier(p.get_source())?;
        let delivery = read_id(p.get_delivery())?;
        let found = self
            .follows
            .withdraw(sender, source, delivery)
            .await
            .map_err(rpc_error)?;
        write_source(results.get().init_source(), &found);
        Ok(())
    }

    async fn sources(
        self: Rc<Self>,
        params: follows::SourcesParams,
        mut results: follows::SourcesResults,
    ) -> Done {
        let group = read_id(request(params.get())?.get_group())?;
        let found = self.follows.sources(group).await.map_err(rpc_error)?;
        write_sources(&found, |n| results.get().init_items(n))
    }

    async fn remove_source(
        self: Rc<Self>,
        params: follows::RemoveSourceParams,
        _: follows::RemoveSourceResults,
    ) -> Done {
        let id = read_id(request(params.get())?.get_id())?;
        self.follows.remove_source(id).await.map_err(rpc_error)
    }
}
