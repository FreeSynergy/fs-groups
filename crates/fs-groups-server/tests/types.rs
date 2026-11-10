//! `Groups.types` over a real socket: the catalog holds every data type and every command, also the ones the
//! program cannot run yet (ADR 0066).

mod common;

use common::with_server;
use fs_groups_core::describe::{COMMANDS, TYPES, registry};
use fs_types::TypeRegistry;

#[tokio::test(flavor = "current_thread")]
async fn types_serves_the_whole_catalog() {
    with_server(|root| async move {
        let catalog = root
            .types_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_catalog()
            .unwrap();
        let answer = catalog.describe_request().send().promise.await.unwrap();
        let read = TypeRegistry::read(answer.get().unwrap().get_types().unwrap()).unwrap();
        assert_eq!(read.len(), TYPES.len());
        let commands: usize = read.iter().map(|t| t.commands.len()).sum();
        assert_eq!(commands, COMMANDS.len());
        for name in TYPES {
            assert_eq!(read.get(name), registry().unwrap().get(name), "{name}");
        }
        let group = read.get("Group").unwrap();
        assert_eq!(group.program, "fs-groups");
        assert!(group.find_command("setJoinPolicy").is_some(), "unsupported commands are described too");
    })
    .await;
}
