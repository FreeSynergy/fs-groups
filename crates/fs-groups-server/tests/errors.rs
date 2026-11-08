//! Failures, the report and the provider rule, as a consumer sees them.

mod common;

use fs_interfaces::rpc::INVALID_REQUEST;
use std::sync::Arc;

use async_trait::async_trait;
use common::{
    create, gp, invitations_of, items_of, problem, store, unknown, with_client, with_server,
};
use fs_groups_core::{
    Cursor, Delegation, Delegations, Group, GroupFeed, GroupListener, GroupRepo, Groups,
    Invitation, Invitations, Membership, Memberships, Page, Result, Subscription,
};
use fs_groups_server::GroupsServer;
use fs_interfaces::rpc::{Problem, TOO_LARGE, write_uuid};
use fs_objects::ObjectId;

#[tokio::test(flavor = "current_thread")]
async fn requests_that_cannot_be_read_are_the_request_s_fault() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut no_id = items.delete_request();
        no_id.get().init_id();
        assert_eq!(
            problem(&no_id.send().promise.await.err().unwrap()),
            (422, INVALID_REQUEST.to_string())
        );

        let mut blank = items.create_request();
        blank.get().set_name("   ");
        assert_eq!(
            problem(&blank.send().promise.await.err().unwrap()),
            (422, INVALID_REQUEST.to_string())
        );

        let mut huge = items.create_request();
        huge.get().set_name("x".repeat(1024 * 1024 + 1).as_str());
        assert_eq!(
            problem(&huge.send().promise.await.err().unwrap()),
            (413, TOO_LARGE.to_string())
        );

        let mut gone = items.rename_request();
        write_uuid(gone.get().init_id(), Some(unknown()));
        gone.get().set_name("x");
        assert_eq!(
            problem(&gone.send().promise.await.err().unwrap()),
            (404, gp("not-found"))
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn add_parent_says_it_is_superseded_and_the_report_says_the_same() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let (a, b) = (create(&items, "A").await, create(&items, "B").await);
        let mut add = items.add_parent_request();
        write_uuid(add.get().init_id(), Some(a));
        write_uuid(add.get().init_parent(), Some(b));
        let e = add.send().promise.await.err().unwrap();
        assert!(Problem::from_error(&e).unwrap().is_unsupported());
        assert_eq!(problem(&e).0, 501);

        let answer = root.support_report_request().send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(
            report.get_provider().unwrap().to_str().unwrap(),
            "fs-groups-core"
        );
        assert_eq!(
            report.get_contract_version().unwrap().to_str().unwrap(),
            "groups 0.1 (draft)"
        );
        let entries: Vec<_> = report
            .get_entries()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.get_capability().unwrap().to_string().unwrap(),
                    e.get_supported(),
                    e.get_reason().unwrap().to_string().unwrap(),
                )
            })
            .collect();
        let offered = [
            "group.create",
            "group.update",
            "group.delete",
            "group.read",
            "group.nest",
            "group.subscribe",
            "member.add",
            "member.remove",
            "member.read",
            "invite.create",
            "invite.answer",
            "invite.read",
            "right.grant",
            "right.read",
            "follow.deliver",
            "follow.read",
            "follow.receive",
        ];
        let mut want: Vec<_> = offered
            .iter()
            .map(|c| ((*c).to_string(), true, String::new()))
            .collect();
        want.push((
            "group.addParent".to_string(),
            false,
            "superseded by requestParent: a parent has to consent to a link".to_string(),
        ));
        assert_eq!(entries, want);
    })
    .await;
}

/// A provider that offers the reads only: real data, and the Null Object defaults for all writes.
struct ReadsOnly(GroupRepo);

#[async_trait]
impl Groups for ReadsOnly {
    fn capabilities(&self) -> fs_interfaces::Capabilities {
        Groups::capabilities(&self.0)
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Group>> {
        Groups::get(&self.0, id).await
    }
    async fn list(&self, cursor: Option<&Cursor>) -> Result<Page<Group>> {
        Groups::list(&self.0, cursor).await
    }
    async fn parents(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Groups::parents(&self.0, id).await
    }
    async fn children(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        Groups::children(&self.0, id).await
    }
}
#[async_trait]
impl Memberships for ReadsOnly {
    async fn direct(&self, group: ObjectId) -> Result<Vec<Membership>> {
        Memberships::direct(&self.0, group).await
    }
}
#[async_trait]
impl Invitations for ReadsOnly {
    async fn list(&self, group: ObjectId) -> Result<Vec<Invitation>> {
        Invitations::list(&self.0, group).await
    }
}
#[async_trait]
impl Delegations for ReadsOnly {
    async fn list(&self, group: ObjectId) -> Result<Vec<Delegation>> {
        Delegations::list(&self.0, group).await
    }
}
impl fs_groups_core::Follows for ReadsOnly {}

impl GroupFeed for ReadsOnly {
    fn subscribe(&self, listener: Arc<dyn GroupListener>) -> Subscription {
        GroupFeed::subscribe(&self.0, listener)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn what_a_provider_does_not_offer_arrives_as_not_possible() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let repo =
                GroupRepo::new(store().await, common::ME).with_ai_marking(Arc::new(common::Humans));
            let g = repo.create("Seeded", "").await.unwrap();
            let server = GroupsServer::new(Arc::new(ReadsOnly(repo)));
            with_client(server.into_client(), |root| async move {
                let items = items_of(&root).await;
                let mut create = items.create_request();
                create.get().set_name("x");
                let e = create.send().promise.await.err().unwrap();
                let p = Problem::from_error(&e).unwrap();
                assert!(p.is_unsupported(), "{p:?}");
                assert_eq!(
                    p.problem_type,
                    "https://freesynergy.net/problems/unsupported"
                );
                assert_eq!(p.status, 501);
                assert_eq!(p.detail, "group.create: this provider does not offer it");

                let invitations = invitations_of(&root).await;
                let mut list = invitations.list_request();
                write_uuid(list.get().init_group(), Some(g.id.as_uuid()));
                let answer = list.send().promise.await.unwrap();
                assert_eq!(
                    answer.get().unwrap().get_items().unwrap().len(),
                    0,
                    "no sweep, still a list"
                );
            })
            .await;
        })
        .await;
}
