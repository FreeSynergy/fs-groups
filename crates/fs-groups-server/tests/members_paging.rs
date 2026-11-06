//! `direct` and `effective` page; identifiers are small; nobody can lock a list.

mod common;

use common::{create, ids, items_of, members_of, nest, problem, with_server};
use fs_interfaces::groups_capnp::memberships;
use fs_interfaces::rpc::{INVALID_REQUEST, TOO_LARGE, read_uuid, write_uuid};
use uuid::Uuid;

const ADA: &str = "https://example.org/people/ada";

async fn add(m: &memberships::Client, group: Uuid, person: &str, role: &str) -> Uuid {
    let mut req = m.add_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().init_member().set_person(person);
    req.get().set_role(role);
    let answer = req.send().promise.await.unwrap();
    read_uuid(
        answer
            .get()
            .unwrap()
            .get_membership()
            .unwrap()
            .get_id()
            .unwrap(),
    )
    .unwrap()
}

async fn is_member(m: &memberships::Client, group: Uuid, person: &str) -> bool {
    let mut req = m.is_member_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_person(person);
    let answer = req.send().promise.await.unwrap();
    answer.get().unwrap().get_member()
}

#[tokio::test(flavor = "current_thread")]
async fn direct_and_effective_page_so_every_id_stays_readable() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let m = members_of(&root).await;
        let (dept, team) = (create(&items, "Dept").await, create(&items, "Team").await);
        nest(&items, team, dept).await;
        let mut made = vec![];
        for n in 0..130 {
            made.push(add(&m, team, &format!("https://example.org/p{n:03}"), "").await);
        }

        let (mut seen, mut pages, mut cursor) = (vec![], vec![], Vec::<u8>::new());
        loop {
            let mut req = m.direct_request();
            write_uuid(req.get().init_group(), Some(team));
            req.get().init_cursor().set_opaque(&cursor);
            let answer = req.send().promise.await.unwrap();
            let r = answer.get().unwrap();
            let list = r.get_items().unwrap();
            pages.push(list.len());
            seen.extend(list.iter().map(|x| read_uuid(x.get_id().unwrap()).unwrap()));
            cursor = r.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!(pages, [100, 30]);
        assert_eq!(seen, made, "every id once, oldest first, across pages");

        let (mut people, mut cursor) = (vec![], Vec::<u8>::new());
        loop {
            let mut req = m.effective_request();
            write_uuid(req.get().init_group(), Some(dept));
            req.get().init_cursor().set_opaque(&cursor);
            let answer = req.send().promise.await.unwrap();
            let r = answer.get().unwrap();
            for x in r.get_items().unwrap() {
                assert_eq!(ids(x.get_via().unwrap()), [dept, team]);
                people.push(x.get_person().unwrap().to_string().unwrap());
            }
            cursor = r.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        let want: Vec<_> = (0..130)
            .map(|n| format!("https://example.org/p{n:03}"))
            .collect();
        assert_eq!(people, want);

        let mut bad = m.direct_request();
        write_uuid(bad.get().init_group(), Some(team));
        bad.get().init_cursor().set_opaque(b"not a cursor");
        let e = bad.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_identifier_is_small_so_nobody_can_lock_a_list() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let m = members_of(&root).await;
        let g = create(&items, "G").await;
        add(&m, g, &"x".repeat(4096), "").await;

        let long = "x".repeat(4097);
        let mut person = m.add_request();
        write_uuid(person.get().init_group(), Some(g));
        person.get().init_member().set_person(long.as_str());
        let e = person.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (413, TOO_LARGE.to_string()));

        let mut role = m.add_request();
        write_uuid(role.get().init_group(), Some(g));
        role.get().init_member().set_person(ADA);
        role.get().set_role(long.as_str());
        let e = role.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (413, TOO_LARGE.to_string()));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn is_member_and_groups_of_trim_the_person() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let m = members_of(&root).await;
        let g = create(&items, "G").await;
        add(&m, g, ADA, "").await;
        let spaced = format!("  {ADA} ");
        assert!(is_member(&m, g, &spaced).await);
        let mut groups = m.groups_of_request();
        groups.get().set_person(spaced.as_str());
        let answer = groups.send().promise.await.unwrap();
        assert_eq!(ids(answer.get().unwrap().get_ids().unwrap()), [g]);
    })
    .await;
}
