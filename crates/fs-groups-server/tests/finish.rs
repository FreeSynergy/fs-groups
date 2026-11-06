//! A client that goes away cannot leave a write half done: the slow backend gives it the time to.

mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::ME;
use fs_groups_core::{GroupRepo, InvitationState, Invitations, Member};
use fs_groups_server::GroupsServer;
use fs_interfaces::groups_capnp::groups;
use fs_interfaces::rpc::{bind_socket, connect, serve, write_uuid};
use fs_store::traits::ObjectRepository;
use fs_store::{Activity, LogEntry, ObjectId, Result, StoredObject, TursoStore};

const STEP: Duration = Duration::from_millis(40);
const INTO_THE_WRITE: Duration = Duration::from_millis(60);
const AFTER_ALL: Duration = Duration::from_millis(900);

/// The real store, every write taking a while.
struct Slow(Arc<TursoStore>);

#[async_trait]
impl ObjectRepository for Slow {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        tokio::time::sleep(STEP).await;
        ObjectRepository::put(&*self.0, object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        tokio::time::sleep(STEP).await;
        ObjectRepository::put_and_log(&*self.0, object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        ObjectRepository::get(&*self.0, id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        tokio::time::sleep(STEP).await;
        ObjectRepository::delete(&*self.0, id, actor).await
    }
}

#[derive(Clone, Copy)]
enum Leave {
    /// The client drops the call.
    Cancel,
    /// The client drops the connection.
    Disconnect,
}

/// Run `call` on a fresh connection, leave after a moment, wait, then let `check` look at the store.
async fn leaves<Call, CallFut, Check, CheckFut>(
    how: Leave,
    seed_and_call: impl AsyncFnOnce(GroupRepo) -> (Call, Check),
) where
    Call: FnOnce(groups::Client) -> CallFut,
    CallFut: Future<Output = ()> + 'static,
    Check: FnOnce() -> CheckFut,
    CheckFut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let store = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo = GroupRepo::with_backend(Arc::new(Slow(store.clone())), store, ME);
            let (call, check) = seed_and_call(repo.clone()).await;

            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("groups.sock");
            let bound = bind_socket(&path).await.unwrap();
            let root = GroupsServer::new(Arc::new(repo)).into_client();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));

            let connection = connect::<groups::Client>(&path).await.unwrap();
            let underway = tokio::task::spawn_local(call(connection.client.clone()));
            tokio::time::sleep(INTO_THE_WRITE).await;
            match how {
                Leave::Cancel => underway.abort(),
                Leave::Disconnect => drop(connection),
            }
            tokio::time::sleep(AFTER_ALL).await;
            check().await;
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

async fn delete_finishes(how: Leave) {
    leaves(how, async |repo: GroupRepo| {
        let g = repo.create("Doomed", "").await.unwrap();
        let people: Vec<_> = (0..6).map(|n| format!("https://example.org/{n}")).collect();
        for p in &people {
            repo.add(g.id, Member::Person(p.clone()), "").await.unwrap();
        }
        let call = move |root: groups::Client| async move {
            let items = common::items_of(&root).await;
            let mut delete = items.delete_request();
            write_uuid(delete.get().init_id(), Some(g.id.as_uuid()));
            let _ = delete.send().promise.await;
        };
        let check = move || async move {
            assert!(
                repo.get(g.id).await.unwrap().is_none(),
                "the group lives on"
            );
            for p in &people {
                assert!(repo.groups_of(p).await.unwrap().is_empty());
            }
        };
        (call, check)
    })
    .await;
}

async fn approve_finishes(how: Leave) {
    leaves(how, async |repo: GroupRepo| {
        let (parent, child) = (
            repo.create("Parent", "").await.unwrap(),
            repo.create("Child", "").await.unwrap(),
        );
        let request = repo.request_parent(child.id, parent.id).await.unwrap();
        let call = move |root: groups::Client| async move {
            let items = common::items_of(&root).await;
            let mut approve = items.approve_parent_request();
            write_uuid(approve.get().init_request(), Some(request.id.as_uuid()));
            let _ = approve.send().promise.await;
        };
        let check = move || async move {
            assert_eq!(repo.parents(child.id).await.unwrap(), [parent.id]);
            let state = repo.parent_requests(parent.id).await.unwrap()[0].state;
            assert_eq!(
                state,
                fs_groups_core::RequestState::Approved,
                "still pending"
            );
        };
        (call, check)
    })
    .await;
}

async fn accept_finishes(how: Leave) {
    leaves(how, async |repo: GroupRepo| {
        let ada = "https://example.org/ada";
        let g = repo.create("G", "").await.unwrap();
        let inv = repo.invite(g.id, ada, "", None).await.unwrap();
        let call = move |root: groups::Client| async move {
            let invitations = common::invitations_of(&root).await;
            let mut accept = invitations.accept_request();
            write_uuid(accept.get().init_id(), Some(inv.id.as_uuid()));
            accept.get().set_person(ada);
            let _ = accept.send().promise.await;
        };
        let check = move || async move {
            assert!(repo.is_member(g.id, ada).await.unwrap());
            let state = Invitations::list(&repo, g.id).await.unwrap()[0].state;
            assert_eq!(
                state,
                InvitationState::Accepted,
                "member, yet still pending"
            );
        };
        (call, check)
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_finishes_when_the_call_is_dropped() {
    delete_finishes(Leave::Cancel).await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_finishes_when_the_connection_is_dropped() {
    delete_finishes(Leave::Disconnect).await;
}

#[tokio::test(flavor = "current_thread")]
async fn approve_finishes_when_the_call_is_dropped() {
    approve_finishes(Leave::Cancel).await;
}

#[tokio::test(flavor = "current_thread")]
async fn accept_finishes_when_the_call_is_dropped() {
    accept_finishes(Leave::Cancel).await;
}

#[tokio::test(flavor = "current_thread")]
async fn accept_finishes_when_the_connection_is_dropped() {
    accept_finishes(Leave::Disconnect).await;
}
