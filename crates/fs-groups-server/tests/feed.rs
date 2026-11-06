//! `subscribe` over a real socket: every kind of write, the envelope as committed, the drop.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use capnp_rpc::new_client;
use common::{create, delegations_of, invitations_of, items_of, members_of, nest, with_server};
use fs_interfaces::common_capnp::{change, change_listener};
use fs_interfaces::groups_capnp::Right;
use fs_interfaces::rpc::{read_envelope, read_uuid, write_uuid};

#[derive(Clone, Default)]
struct Seen(Rc<RefCell<Vec<String>>>);

impl change_listener::Server for Seen {
    async fn change(
        self: Rc<Self>,
        params: change_listener::ChangeParams,
    ) -> Result<(), capnp::Error> {
        let text = match params.get()?.get_change()?.which()? {
            change::Which::Created(e) => {
                let e = read_envelope(e?)?;
                format!("created {} {}", e.object_type, e.name)
            }
            change::Which::Updated(e) => {
                let e = read_envelope(e?)?;
                format!("updated {} {}", e.object_type, e.name)
            }
            change::Which::Deleted(id) => format!("deleted {}", read_uuid(id?).unwrap()),
        };
        self.0.borrow_mut().push(text);
        Ok(())
    }
}

impl Seen {
    async fn take(&self) -> Vec<String> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::mem::take(&mut self.0.borrow_mut())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn subscribers_see_every_write_until_they_drop_the_capability() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let seen = Seen::default();
        let listener: change_listener::Client = new_client(seen.clone());
        let mut sub = root.subscribe_request();
        sub.get().set_listener(listener);
        let answer = sub.send().promise.await.unwrap();
        let subscription = answer.get().unwrap().get_subscription().unwrap();

        let g = create(&items, "Watched").await;
        assert_eq!(seen.take().await, ["created Group Watched"]);

        let mut rename = items.rename_request();
        write_uuid(rename.get().init_id(), Some(g));
        rename.get().set_name("Seen");
        rename.send().promise.await.unwrap();
        assert_eq!(seen.take().await, ["updated Group Seen"]);

        let child = create(&items, "Child").await;
        nest(&items, child, g).await;
        let changes = seen.take().await;
        assert!(
            changes
                .iter()
                .any(|c| c.starts_with("created ParentRequest")),
            "{changes:?}"
        );
        assert!(
            changes.iter().any(|c| c == "updated Group Child"),
            "{changes:?}"
        );

        let mut add = members_of(&root).await.add_request();
        write_uuid(add.get().init_group(), Some(g));
        add.get().init_member().set_person("https://example.org/a");
        add.send().promise.await.unwrap();
        assert_eq!(
            seen.take().await,
            ["created Relationship https://example.org/a in Seen"]
        );

        let mut grant = delegations_of(&root).await.grant_request();
        write_uuid(grant.get().init_group(), Some(g));
        grant
            .get()
            .init_holder()
            .set_person("https://example.org/a");
        grant.get().set_right(Right::Read);
        grant.send().promise.await.unwrap();
        assert_eq!(
            seen.take().await,
            ["created Delegation https://example.org/a may read Seen"]
        );

        let mut invite = invitations_of(&root).await.invite_request();
        write_uuid(invite.get().init_group(), Some(g));
        invite.get().set_invitee("https://example.org/b");
        invite.send().promise.await.unwrap();
        assert_eq!(
            seen.take().await,
            ["created Invite invitation of https://example.org/b to Seen"]
        );

        let mut delete = items.delete_request();
        write_uuid(delete.get().init_id(), Some(g));
        delete.send().promise.await.unwrap();
        let changes = seen.take().await;
        assert_eq!(
            changes.last().cloned(),
            Some(format!("deleted {g}")),
            "{changes:?}"
        );
        let deleted = changes.iter().filter(|c| c.starts_with("deleted ")).count();
        assert_eq!(
            deleted, 5,
            "membership, delegation, invitation, request, group: {changes:?}"
        );
        assert!(
            changes.contains(&"updated Group Child".to_string()),
            "the child loses its link: {changes:?}"
        );

        drop(subscription);
        drop(answer);
        tokio::time::sleep(Duration::from_millis(100)).await;
        create(&items, "Unwatched").await;
        assert!(seen.take().await.is_empty(), "still told after the drop");
    })
    .await;
}
