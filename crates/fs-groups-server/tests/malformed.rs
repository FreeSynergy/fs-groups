//! A request whose pointers are of the wrong kind is the request's fault: typed, connection alive.
//! Each call sends the parameters of another method, so a field holds what it must not.

mod common;

use capnp::any_pointer;
use capnp::capability::Client;
use capnp::traits::HasTypeId;
use common::{
    create, delegations_of, invitations_of, items_of, members_of, problem, unknown, with_server,
};
use fs_interfaces::common_capnp::envelope;
use fs_interfaces::groups_capnp::{delegations, groups, invitations, items, memberships};
use fs_interfaces::rpc::{INVALID_REQUEST, write_uuid};

/// A request carrying `P` for the method `method` of the interface `id`.
fn call<P: capnp::traits::Owned>(
    client: &Client,
    id: u64,
    method: u16,
) -> capnp::capability::Request<P, any_pointer::Owned> {
    client.new_call(id, method, None)
}

async fn refused(
    request: capnp::capability::Request<impl capnp::traits::Owned, any_pointer::Owned>,
) {
    let e = request.send().promise.await.err().unwrap();
    assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()), "{e}");
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_pointers_are_invalid_requests() {
    with_server(|root| async move {
        const ITEMS: u64 = <items::Client as HasTypeId>::TYPE_ID;
        let it = items_of(&root).await;

        // id, as text
        let mut r = call::<items::create_params::Owned>(&it.client, ITEMS, 3);
        r.get().set_name("not an id");
        refused(r).await;
        // name, as a struct
        let mut r = call::<items::rename_params::Owned>(&it.client, ITEMS, 0);
        write_uuid(r.get().init_id(), Some(unknown()));
        refused(r).await;
        // cursor, as text
        let mut r = call::<items::create_params::Owned>(&it.client, ITEMS, 5);
        r.get().set_name("not a cursor");
        refused(r).await;

        // member and holder, as text
        let m = members_of(&root).await;
        let mut r = call::<memberships::is_member_params::Owned>(
            &m.client,
            <memberships::Client as HasTypeId>::TYPE_ID,
            0,
        );
        write_uuid(r.get().init_group(), Some(unknown()));
        r.get().set_person("not a member");
        refused(r).await;
        let d = delegations_of(&root).await;
        let mut r = call::<memberships::is_member_params::Owned>(
            &d.client,
            <delegations::Client as HasTypeId>::TYPE_ID,
            0,
        );
        write_uuid(r.get().init_group(), Some(unknown()));
        r.get().set_person("not a holder");
        refused(r).await;

        // expires, as text (an envelope's first pointers are an id and three texts)
        let i = invitations_of(&root).await;
        let mut r =
            call::<envelope::Owned>(&i.client, <invitations::Client as HasTypeId>::TYPE_ID, 0);
        write_uuid(r.get().init_id(), Some(unknown()));
        r.get().set_iri("https://example.org/ada");
        r.get().set_type("member");
        r.get().set_name("not a time");
        refused(r).await;

        // listener, as text
        let mut r = call::<items::create_params::Owned>(
            &root.client,
            <groups::Client as HasTypeId>::TYPE_ID,
            5,
        );
        r.get().set_name("not a capability");
        refused(r).await;

        create(&it, "Still connected").await;
    })
    .await;
}
