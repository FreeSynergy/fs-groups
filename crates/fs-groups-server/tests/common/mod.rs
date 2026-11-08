//! Shared helpers of the integration tests: a real server on a real Unix socket, real store.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_groups_core::GroupRepo;
use fs_groups_server::GroupsServer;
use fs_interfaces::groups_capnp::{delegations, groups, invitations, items, memberships};
use fs_interfaces::rpc::{
    Problem, bind_socket, connect, read_envelope, read_uuid, serve, write_uuid,
};
use fs_objects::{Store, TursoStore};
use uuid::Uuid;

pub const ME: &str = "https://local.invalid/users/owner";

pub async fn store() -> Arc<dyn Store> {
    Arc::new(TursoStore::open_in_memory().await.unwrap())
}

/// Serve `root` on a socket and run the test on a connection to it.
pub async fn with_client<F, Fut>(root: groups::Client, test: F)
where
    F: FnOnce(groups::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("groups.sock");
    let bound = bind_socket(&path).await.unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::task::spawn_local(serve(bound, root.client, async {
        let _ = stopped.await;
    }));
    let connection = connect::<groups::Client>(&path).await.unwrap();
    test(connection.client.clone()).await;
    drop(connection);
    let _ = stop.send(());
    serving.await.unwrap().unwrap();
}

/// The store-backed provider behind a socket.
pub async fn with_server<F, Fut>(test: F)
where
    F: FnOnce(groups::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let repo =
                Arc::new(GroupRepo::new(store().await, ME).with_ai_marking(Arc::new(Humans)));
            with_client(GroupsServer::new(repo).into_client(), test).await;
        })
        .await;
}

macro_rules! capability {
    ($name:ident, $request:ident, $getter:ident, $client:ty) => {
        pub async fn $name(root: &groups::Client) -> $client {
            let answer = root.$request().send().promise.await.unwrap();
            answer.get().unwrap().$getter().unwrap()
        }
    };
}
capability!(items_of, items_request, get_items, items::Client);
capability!(
    members_of,
    memberships_request,
    get_memberships,
    memberships::Client
);
capability!(
    invitations_of,
    invitations_request,
    get_invitations,
    invitations::Client
);
capability!(
    delegations_of,
    delegations_request,
    get_delegations,
    delegations::Client
);

/// Create a group and answer its id.
pub async fn create(items: &items::Client, name: &str) -> Uuid {
    let mut req = items.create_request();
    req.get().set_name(name);
    req.get().set_kind("team");
    let answer = req.send().promise.await.unwrap();
    let group = answer.get().unwrap().get_group().unwrap();
    read_envelope(group.get_envelope().unwrap())
        .unwrap()
        .id
        .as_uuid()
}

/// Ask for a link; answers the request id.
pub async fn request(items: &items::Client, child: Uuid, parent: Uuid) -> Uuid {
    let mut req = items.request_parent_request();
    write_uuid(req.get().init_child(), Some(child));
    write_uuid(req.get().init_parent(), Some(parent));
    let answer = req.send().promise.await.unwrap();
    let id = answer
        .get()
        .unwrap()
        .get_request()
        .unwrap()
        .get_id()
        .unwrap();
    read_uuid(id).unwrap()
}

/// Ask for a link and approve it.
pub async fn nest(items: &items::Client, child: Uuid, parent: Uuid) {
    let id = request(items, child, parent).await;
    let mut req = items.approve_parent_request();
    write_uuid(req.get().init_request(), Some(id));
    req.send().promise.await.unwrap();
}

/// The ids of an answer's list.
pub fn ids(
    list: capnp::struct_list::Reader<'_, fs_interfaces::common_capnp::uuid::Owned>,
) -> Vec<Uuid> {
    list.iter().map(|u| read_uuid(u).unwrap()).collect()
}

/// A fresh id nothing has.
pub fn unknown() -> Uuid {
    fs_objects::ObjectId::new().as_uuid()
}

/// The problem type URI of this program's `name`.
pub fn gp(name: &str) -> String {
    format!("https://freesynergy.net/problems/groups/{name}")
}

/// The status and the exact problem type of a failed call.
pub fn problem(e: &capnp::Error) -> (u16, String) {
    let p = Problem::from_error(e).unwrap_or_else(|| panic!("no problem in {e}"));
    (p.status, p.problem_type)
}

/// Everybody in these tests is a person.
pub struct Humans;

#[async_trait::async_trait]
impl fs_groups_core::AiMarking for Humans {
    async fn is_ai(&self, _person: &str) -> fs_groups_core::Result<Option<bool>> {
        Ok(Some(false))
    }
}
