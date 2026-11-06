//! `Groups` root interface: support report, the four sub-capabilities, subscribe.
//!
//! Pattern: Adapter + Factory — the root hands out `Items`, `Memberships`, `Invitations` and
//! `Delegations`. [`GroupsServer`] is what a program serves; it depends on the traits, never on
//! `GroupRepo` (ADR 0043). The transport says who connected only as far as "this user"; no method
//! of the contract names a verified caller (`DESIGN.md`).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_groups_core::{Delegations, GroupFeed, Groups, Invitations, Memberships};
use fs_interfaces::groups_capnp::groups;

use crate::delegations_service::DelegationsImpl;
use crate::feed::subscribe;
use crate::invitations_service::InvitationsImpl;
use crate::items_service::ItemsImpl;
use crate::memberships_service::MembershipsImpl;
use crate::nesting_service::ADD_PARENT;

/// The version named in the header of `groups.capnp`.
const CONTRACT_VERSION: &str = "groups 0.1 (draft)";

/// The root capability of the groups program.
pub struct GroupsServer {
    groups: Arc<dyn Groups>,
    memberships: Arc<dyn Memberships>,
    invitations: Arc<dyn Invitations>,
    delegations: Arc<dyn Delegations>,
    feed: Arc<dyn GroupFeed>,
}

impl GroupsServer {
    /// Serve this provider.
    pub fn new<P>(provider: Arc<P>) -> Self
    where
        P: Groups + Memberships + Invitations + Delegations + GroupFeed + 'static,
    {
        Self {
            groups: provider.clone(),
            memberships: provider.clone(),
            invitations: provider.clone(),
            delegations: provider.clone(),
            feed: provider,
        }
    }

    /// The capability to hand to `fs_interfaces::rpc::serve`.
    #[must_use]
    pub fn into_client(self) -> groups::Client {
        new_client(self)
    }
}

type Done = Result<(), capnp::Error>;

impl groups::Server for GroupsServer {
    async fn support_report(
        self: Rc<Self>,
        _: groups::SupportReportParams,
        mut results: groups::SupportReportResults,
    ) -> Done {
        let mut report = self.groups.capabilities();
        report.unsupported(ADD_PARENT.0, ADD_PARENT.1);
        report.write(CONTRACT_VERSION, results.get().init_report());
        Ok(())
    }

    async fn items(
        self: Rc<Self>,
        _: groups::ItemsParams,
        mut results: groups::ItemsResults,
    ) -> Done {
        let items = ItemsImpl {
            groups: self.groups.clone(),
        };
        results.get().set_items(new_client(items));
        Ok(())
    }

    async fn memberships(
        self: Rc<Self>,
        _: groups::MembershipsParams,
        mut results: groups::MembershipsResults,
    ) -> Done {
        let memberships = MembershipsImpl {
            memberships: self.memberships.clone(),
        };
        results.get().set_memberships(new_client(memberships));
        Ok(())
    }

    async fn invitations(
        self: Rc<Self>,
        _: groups::InvitationsParams,
        mut results: groups::InvitationsResults,
    ) -> Done {
        let invitations = InvitationsImpl {
            invitations: self.invitations.clone(),
        };
        results.get().set_invitations(new_client(invitations));
        Ok(())
    }

    async fn delegations(
        self: Rc<Self>,
        _: groups::DelegationsParams,
        mut results: groups::DelegationsResults,
    ) -> Done {
        let delegations = DelegationsImpl {
            delegations: self.delegations.clone(),
        };
        results.get().set_delegations(new_client(delegations));
        Ok(())
    }

    async fn subscribe(
        self: Rc<Self>,
        params: groups::SubscribeParams,
        mut results: groups::SubscribeResults,
    ) -> Done {
        let listener = params.get()?.get_listener()?;
        results
            .get()
            .set_subscription(subscribe(&self.feed, listener));
        Ok(())
    }
}
