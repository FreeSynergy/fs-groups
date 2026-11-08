//! The item methods of the contract over a real socket, against the store-backed provider.

mod common;

use common::{create, items_of, with_server};
use fs_interfaces::groups_capnp::{RightFlow, items};
use fs_interfaces::rpc::{read_envelope, write_uuid};
use uuid::Uuid;

/// Long enough for the next write to have a later timestamp.
const STORE_TICK: std::time::Duration = std::time::Duration::from_millis(5);

async fn get_found(items: &items::Client, id: Uuid) -> bool {
    let mut get = items.get_request();
    write_uuid(get.get().init_id(), Some(id));
    let answer = get.send().promise.await.unwrap();
    answer.get().unwrap().get_found()
}

#[tokio::test(flavor = "current_thread")]
async fn create_get_rename_kind_flow_delete() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "First").await;

        let mut get = items.get_request();
        write_uuid(get.get().init_id(), Some(id));
        let got = get.send().promise.await.unwrap();
        let group = got.get().unwrap().get_group().unwrap();
        let env = read_envelope(group.get_envelope().unwrap()).unwrap();
        assert_eq!(
            (
                env.name.as_str(),
                env.object_type.as_str(),
                env.context.as_deref()
            ),
            ("First", "Group", Some("groups"))
        );
        assert_eq!(env.attributed_to.as_deref(), Some(common::ME));
        assert_eq!(group.get_kind().unwrap().to_str().unwrap(), "team");
        assert_eq!(group.get_right_flow().unwrap(), RightFlow::None);
        let as2 = group.get_as2().unwrap().to_str().unwrap();
        assert!(
            as2.contains("\"type\":\"Group\"") && as2.contains("First"),
            "{as2}"
        );

        tokio::time::sleep(STORE_TICK).await;
        let mut rename = items.rename_request();
        write_uuid(rename.get().init_id(), Some(id));
        rename.get().set_name("Renamed");
        let renamed = rename.send().promise.await.unwrap();
        let g = renamed.get().unwrap().get_group().unwrap();
        let after = read_envelope(g.get_envelope().unwrap()).unwrap();
        assert_eq!(after.name, "Renamed");
        assert!(after.updated > env.updated);
        assert_eq!(after.published, env.published, "the envelope as stored");

        tokio::time::sleep(STORE_TICK).await;
        let mut kind = items.set_kind_request();
        write_uuid(kind.get().init_id(), Some(id));
        kind.get().set_kind("project");
        let changed = kind.send().promise.await.unwrap();
        let g = changed.get().unwrap().get_group().unwrap();
        assert_eq!(g.get_kind().unwrap().to_str().unwrap(), "project");
        let kinded = read_envelope(g.get_envelope().unwrap()).unwrap();
        assert!(
            kinded.updated > after.updated,
            "the kind answer is as stored"
        );
        assert_eq!(kinded.published, env.published);
        tokio::time::sleep(STORE_TICK).await;

        let mut flow = items.set_right_flow_request();
        write_uuid(flow.get().init_id(), Some(id));
        flow.get().set_flow(RightFlow::Down);
        let changed = flow.send().promise.await.unwrap();
        let g = changed.get().unwrap().get_group().unwrap();
        assert_eq!(g.get_right_flow().unwrap(), RightFlow::Down);
        let flowed = read_envelope(g.get_envelope().unwrap()).unwrap();
        assert!(
            flowed.updated > kinded.updated,
            "the flow answer is as stored"
        );
        assert_eq!(flowed.published, env.published);
        assert!(
            g.get_as2()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("\"fs:rightFlow\":\"down\"")
        );

        assert!(get_found(&items, id).await);
        let mut delete = items.delete_request();
        write_uuid(delete.get().init_id(), Some(id));
        delete.send().promise.await.unwrap();
        assert!(!get_found(&items, id).await);
    })
    .await;
}
