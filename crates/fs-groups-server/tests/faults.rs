//! What a failing provider looks like to a consumer: store failures by variant, an answer too large
//! for a message, a failing sweep. The real repository holds the data; only the failures are planted.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{
    create, gp, invitations_of, items_of, members_of, problem, store, unknown, with_client,
};
use fs_groups_core::{
    Cursor, Delegation, Delegations, Error, Group, GroupFeed, GroupListener, GroupRepo, Groups,
    Invitation, Invitations, Member, Membership, Memberships, Page, Result, Right, RightFlow,
    Subscription,
};
use fs_groups_server::GroupsServer;
use fs_interfaces::rpc::{INVALID_REQUEST, Problem, TOO_LARGE, write_uuid};
use fs_store::{ObjectId, Timestamp};

struct Faulty(GroupRepo);

fn store_error<T>(e: fs_store::Error) -> Result<T> {
    Err(Error::Store(e))
}

#[async_trait]
impl Groups for Faulty {
    fn capabilities(&self) -> fs_interfaces::Capabilities {
        Groups::capabilities(&self.0)
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Group>> {
        store_error(fs_store::Error::NotFound(id))
    }
    async fn list(&self, _: Option<&Cursor>) -> Result<Page<Group>> {
        store_error(fs_store::Error::Backend("disk gone".into()))
    }
    async fn parents(&self, id: ObjectId) -> Result<Vec<ObjectId>> {
        store_error(fs_store::Error::OriginalImmutable(id))
    }
    async fn children(&self, _: ObjectId) -> Result<Vec<ObjectId>> {
        store_error(fs_store::Error::Conflict("changed".into()))
    }
    async fn rename(&self, _: ObjectId, _: &str) -> Result<Group> {
        store_error(fs_store::Error::Invalid("no".into()))
    }
    async fn create(&self, name: &str, kind: &str) -> Result<Group> {
        Groups::create(&self.0, name, kind).await
    }
    async fn set_right_flow(&self, id: ObjectId, _: RightFlow) -> Result<Group> {
        store_error(fs_store::Error::EncryptionRequired(id))
    }
}

#[async_trait]
impl Memberships for Faulty {
    async fn direct(&self, _: ObjectId) -> Result<Vec<Membership>> {
        Err(Error::Unmapped(vec!["ada".into()]))
    }
    async fn remove(&self, _: ObjectId) -> Result<()> {
        Err(Error::Directory("refused".into()))
    }
}

#[async_trait]
impl Invitations for Faulty {
    async fn list(&self, group: ObjectId) -> Result<Vec<Invitation>> {
        Invitations::list(&self.0, group).await
    }
    async fn expire_due(&self, _: Timestamp) -> Result<usize> {
        store_error(fs_store::Error::Backend("disk gone".into()))
    }
}

#[async_trait]
impl Delegations for Faulty {
    /// More than one answer can hold, in a list the contract cannot page.
    async fn list(&self, group: ObjectId) -> Result<Vec<Delegation>> {
        let person = "x".repeat(4096);
        Ok((0..9000)
            .map(|_| Delegation {
                id: ObjectId::new(),
                group,
                holder: Member::Person(person.clone()),
                right: Right::Read,
            })
            .collect())
    }
}

impl GroupFeed for Faulty {
    fn subscribe(&self, listener: Arc<dyn GroupListener>) -> Subscription {
        GroupFeed::subscribe(&self.0, listener)
    }
}

async fn with_faulty<F, Fut>(test: F)
where
    F: FnOnce(fs_interfaces::groups_capnp::groups::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let repo = GroupRepo::new(store().await, common::ME);
            let server = GroupsServer::new(Arc::new(Faulty(repo)));
            with_client(server.into_client(), test).await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn store_failures_arrive_by_what_they_mean() {
    with_faulty(|root| async move {
        let items = items_of(&root).await;
        let id = unknown();

        let mut get = items.get_request();
        write_uuid(get.get().init_id(), Some(id));
        let e = get.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (404, gp("not-found")));

        let mut rename = items.rename_request();
        write_uuid(rename.get().init_id(), Some(id));
        let e = rename.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()));

        let mut parents = items.parents_request();
        write_uuid(parents.get().init_id(), Some(id));
        let e = parents.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (409, gp("conflict")));

        let mut children = items.children_request();
        write_uuid(children.get().init_id(), Some(id));
        let e = children.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (409, gp("conflict")));

        // Failures of this program, not of the request: no 4xx, no problem type to act on.
        let e = items.list_request().send().promise.await.err().unwrap();
        assert!(Problem::from_error(&e).is_none(), "{e}");
        assert!(e.extra.contains("disk gone"), "{e}");
        let mut flow = items.set_right_flow_request();
        write_uuid(flow.get().init_id(), Some(id));
        let e = flow.send().promise.await.err().unwrap();
        assert!(Problem::from_error(&e).is_none(), "{e}");

        let m = members_of(&root).await;
        let mut direct = m.direct_request();
        write_uuid(direct.get().init_group(), Some(id));
        let e = direct.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (502, gp("unmapped")));
        let mut remove = m.remove_request();
        write_uuid(remove.get().init_membership(), Some(id));
        let e = remove.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (502, gp("directory")));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_sweep_that_fails_fails_the_list_instead_of_answering_stale_states() {
    with_faulty(|root| async move {
        let items = items_of(&root).await;
        let g = create(&items, "G").await;
        let i = invitations_of(&root).await;
        let mut list = i.list_request();
        write_uuid(list.get().init_group(), Some(g));
        let e = list.send().promise.await.err().unwrap();
        assert!(Problem::from_error(&e).is_none(), "{e}");
        assert!(e.extra.contains("disk gone"), "{e}");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_list_without_a_cursor_answers_too_large_and_the_connection_lives() {
    with_faulty(|root| async move {
        let items = items_of(&root).await;
        let g = create(&items, "G").await;
        let d = common::delegations_of(&root).await;
        let mut list = d.list_request();
        write_uuid(list.get().init_group(), Some(g));
        let e = list.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (413, TOO_LARGE.to_string()));
        create(&items, "Still connected").await;
    })
    .await;
}
