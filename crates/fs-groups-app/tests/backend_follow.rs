//! The Admin backend over a real socket: following between groups.

mod common;

use common::{ME, World, id_of, values};
use fs_admin_frontend::{Backend, Values};
use fs_groups_core::describe::{DELEGATION, DELIVERY, GROUP, SOURCE};

#[test]
fn following_deliver_receive_withdraw() {
    let w = World::new();
    let g = id_of(
        w.of(GROUP)
            .run("create", None, &values(&[("name", &["Giver"])]))
            .unwrap(),
    );
    // a group gives only rights it holds: the acting person holds manage here
    w.of(DELEGATION)
        .run(
            "grant",
            None,
            &values(&[("group", &[&g]), ("holder", &[ME]), ("right", &["manage"])]),
        )
        .unwrap();
    let deliveries = w.of(DELIVERY);
    let d = id_of(
        deliveries
            .run(
                "deliver",
                None,
                &values(&[
                    ("group", &[&g]),
                    ("recipient", &["https://other.invalid/groups/abc"]),
                    ("rights", &["read"]),
                    ("passOn", &["no"]),
                ]),
            )
            .unwrap(),
    );
    let rows = deliveries.list(&values(&[("group", &[&g])])).unwrap();
    assert_eq!(
        (rows[0].id.as_str(), rows[0].values["state"][0].as_str()),
        (d.as_str(), "active")
    );
    deliveries.run("revoke", Some(&d), &Values::new()).unwrap();
    assert_eq!(
        deliveries.list(&Values::new()).unwrap()[0].values["state"],
        ["revoked"]
    );

    // the receiving side, the offer comes from another origin
    let receiver = id_of(
        w.of(GROUP)
            .run("create", None, &values(&[("name", &["Receiver"])]))
            .unwrap(),
    );
    let sources = w.of(SOURCE);
    let recipient = format!("https://local.invalid/groups/{receiver}");
    let s = id_of(
        sources
            .run(
                "receive",
                None,
                &values(&[
                    ("sender", &["other.invalid"]),
                    ("delivery", &[&d]),
                    ("source", &["https://other.invalid/groups/abc"]),
                    ("recipient", &[&recipient]),
                    ("rights", &["read"]),
                    ("passOn", &["no"]),
                ]),
            )
            .unwrap(),
    );
    let found = sources.list(&Values::new()).unwrap();
    assert!(found.iter().any(|r| r.id == s));
    // the giver revoked: the receiver's entry is withdrawn
    let w2 = sources
        .run(
            "withdraw",
            None,
            &values(&[
                ("sender", &["other.invalid"]),
                ("source", &["https://other.invalid/groups/abc"]),
                ("delivery", &[&d]),
            ]),
        )
        .unwrap();
    assert_eq!(w2.id.as_deref(), Some(s.as_str()));
    let state = sources
        .list(&Values::new())
        .unwrap()
        .into_iter()
        .find(|r| r.id == s)
        .unwrap();
    assert_eq!(state.values["state"], ["withdrawn"]);
    sources.run("remove", Some(&s), &Values::new()).unwrap();
}
