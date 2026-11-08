//! The real program: started, used, stopped and started again; everything is still there. The
//! `--actor` decides who may approve, which is exactly why it is not an authentication.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::groups_capnp::{groups, items};
use fs_interfaces::rpc::{Connection, Problem, connect, read_envelope, read_uuid, write_uuid};
use uuid::Uuid;

const OWNER: &str = "https://example.org/users/owner";
const OTHER: &str = "https://example.org/users/other";

fn start(socket: &Path, data: &Path, actor: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_fs-groups-server"))
        .arg("--socket")
        .arg(socket)
        .arg("--data")
        .arg(data)
        .arg("--actor")
        .arg(actor)
        .arg("--instance")
        .arg("example.org")
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

async fn wait_for(socket: &Path) {
    for _ in 0..400 {
        if tokio::net::UnixStream::connect(socket).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server did not come up");
}

fn terminate(mut child: Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
}

async fn items_on(connection: &Connection<groups::Client>) -> items::Client {
    let answer = connection
        .client
        .items_request()
        .send()
        .promise
        .await
        .unwrap();
    answer.get().unwrap().get_items().unwrap()
}

async fn create(items: &items::Client, name: &str) -> Uuid {
    let mut req = items.create_request();
    req.get().set_name(name);
    let answer = req.send().promise.await.unwrap();
    let group = answer.get().unwrap().get_group().unwrap();
    let envelope = read_envelope(group.get_envelope().unwrap()).unwrap();
    assert_eq!(envelope.attributed_to.as_deref(), Some(OWNER));
    let iri = format!("https://example.org/groups/{}", envelope.id);
    assert_eq!(envelope.iri.as_deref(), Some(iri.as_str()));
    envelope.id.as_uuid()
}

fn approve(
    items: &items::Client,
    request: Uuid,
) -> capnp::capability::RemotePromise<items::approve_parent_results::Owned> {
    let mut req = items.approve_parent_request();
    write_uuid(req.get().init_request(), Some(request));
    req.send()
}

#[tokio::test(flavor = "current_thread")]
async fn groups_survive_a_restart_and_a_second_start_is_refused() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("groups.sock");
            let data = dir.path().join("data");

            let first = start(&socket, &data, OWNER);
            wait_for(&socket).await;
            let mode = std::fs::metadata(&data).unwrap().mode() & 0o777;
            assert_eq!(mode, 0o700, "the data directory is this user's alone");
            let mut second = start(&socket, &data, OWNER);
            assert_eq!(
                second.wait().unwrap().code(),
                Some(1),
                "a second server started"
            );
            assert!(socket.exists(), "the second start touched the first");

            let connection = connect::<groups::Client>(&socket).await.unwrap();
            let items = items_on(&connection).await;
            let (parent, child) = (
                create(&items, "Parent").await,
                create(&items, "Child").await,
            );
            let mut ask = items.request_parent_request();
            write_uuid(ask.get().init_child(), Some(child));
            write_uuid(ask.get().init_parent(), Some(parent));
            let asked = ask.send().promise.await.unwrap();
            let request = read_uuid(
                asked
                    .get()
                    .unwrap()
                    .get_request()
                    .unwrap()
                    .get_id()
                    .unwrap(),
            )
            .unwrap();
            drop((asked, items, connection));
            terminate(first);
            assert!(!socket.exists(), "the socket stayed behind");

            let other = start(&socket, &data, OTHER);
            wait_for(&socket).await;
            let connection = connect::<groups::Client>(&socket).await.unwrap();
            let items = items_on(&connection).await;
            let e = approve(&items, request).promise.await.err().unwrap();
            let p = Problem::from_error(&e).unwrap();
            assert_eq!(p.status, 403, "{p:?}");
            assert!(p.problem_type.ends_with("/forbidden"));
            drop((items, connection));
            terminate(other);

            let again = start(&socket, &data, OWNER);
            wait_for(&socket).await;
            let connection = connect::<groups::Client>(&socket).await.unwrap();
            let items = items_on(&connection).await;
            approve(&items, request).promise.await.unwrap();
            let listed = items.list_request().send().promise.await.unwrap();
            let page = listed.get().unwrap().get_page().unwrap();
            assert_eq!(page.get_items().unwrap().len(), 2);
            let mut parents = items.parents_request();
            write_uuid(parents.get().init_id(), Some(child));
            let answer = parents.send().promise.await.unwrap();
            let found = answer.get().unwrap().get_ids().unwrap();
            assert_eq!(read_uuid(found.get(0)), Some(parent));
            drop((answer, listed, items, connection));
            terminate(again);
        })
        .await;
}

#[test]
fn exit_codes_say_what_went_wrong() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_fs-groups-server"))
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code()
    };
    assert_eq!(run(&["--help"]), Some(0));
    assert_eq!(run(&["--nonsense"]), Some(2));
    assert_eq!(run(&["--socket"]), Some(2));

    let dir = tempfile::tempdir().unwrap();
    let open = dir.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, PermissionsExt::from_mode(0o755)).unwrap();
    let socket = dir.path().join("groups.sock");
    let code = run(&[
        "--socket",
        socket.to_str().unwrap(),
        "--data",
        open.to_str().unwrap(),
    ]);
    assert_eq!(
        code,
        Some(1),
        "a data directory others can reach is refused"
    );
    assert!(!open.join("objects.db").exists());
}
