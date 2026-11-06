//! The `Memberships` interface over a real socket.

mod common;

use common::{create, gp, ids, items_of, members_of, nest, problem, unknown, with_server};
use fs_interfaces::groups_capnp::{member, memberships};
use fs_interfaces::rpc::INVALID_REQUEST;
use fs_interfaces::rpc::{TOO_LARGE, read_uuid, write_uuid};
use uuid::Uuid;

const ADA: &str = "https://example.org/people/ada";

async fn add(m: &memberships::Client, group: Uuid, person: &str, role: &str) -> Uuid {
    let mut req = m.add_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().init_member().set_person(person);
    req.get().set_role(role);
    let answer = req.send().promise.await.unwrap();
    let membership = answer.get().unwrap().get_membership().unwrap();
    assert_eq!(read_uuid(membership.get_group().unwrap()), Some(group));
    assert_eq!(membership.get_role().unwrap().to_str().unwrap(), role);
    read_uuid(membership.get_id().unwrap()).unwrap()
}

async fn is_member(m: &memberships::Client, group: Uuid, person: &str) -> bool {
    let mut req = m.is_member_request();
    write_uuid(req.get().init_group(), Some(group));
    req.get().set_person(person);
    let answer = req.send().promise.await.unwrap();
    answer.get().unwrap().get_member()
}

#[tokio::test(flavor = "current_thread")]
async fn direct_effective_groups_of_and_remove() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let m = members_of(&root).await;
        let (dept, team) = (create(&items, "Dept").await, create(&items, "Team").await);
        nest(&items, team, dept).await;

        let other = create(&items, "Other").await;
        add(&m, other, "https://example.org/people/bo", "").await;

        let id = add(&m, team, ADA, "lead").await;
        assert_eq!(
            add(&m, team, ADA, "lead").await,
            id,
            "the same triple is one membership"
        );

        let mut direct = m.direct_request();
        write_uuid(direct.get().init_group(), Some(team));
        let answer = direct.send().promise.await.unwrap();
        let list = answer.get().unwrap().get_items().unwrap();
        assert_eq!(list.len(), 1, "bo of another group stays out");
        assert_eq!(read_uuid(list.get(0).get_id().unwrap()), Some(id));
        assert_eq!(list.get(0).get_role().unwrap().to_str().unwrap(), "lead");
        match list.get(0).get_member().unwrap().which().unwrap() {
            member::Which::Person(p) => assert_eq!(p.unwrap().to_str().unwrap(), ADA),
            member::Which::Group(_) => panic!("a group"),
        }

        let mut effective = m.effective_request();
        write_uuid(effective.get().init_group(), Some(dept));
        let answer = effective.send().promise.await.unwrap();
        let list = answer.get().unwrap().get_items().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list.get(0).get_person().unwrap().to_str().unwrap(), ADA);
        assert_eq!(ids(list.get(0).get_via().unwrap()), [dept, team]);

        let mut groups = m.groups_of_request();
        groups.get().set_person(ADA);
        let answer = groups.send().promise.await.unwrap();
        let mut found = ids(answer.get().unwrap().get_ids().unwrap());
        found.sort();
        let mut want = vec![dept, team];
        want.sort();
        assert_eq!(found, want);

        assert!(is_member(&m, dept, ADA).await);
        assert!(!is_member(&m, dept, "https://example.org/people/bo").await);

        let mut remove = m.remove_request();
        write_uuid(remove.get().init_membership(), Some(id));
        remove.send().promise.await.unwrap();
        assert!(!is_member(&m, team, ADA).await);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn refusals_of_the_contract() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let m = members_of(&root).await;
        let (a, b) = (create(&items, "A").await, create(&items, "B").await);

        let mut group_member = m.add_request();
        write_uuid(group_member.get().init_group(), Some(a));
        write_uuid(group_member.get().init_member().init_group(), Some(b));
        let e = group_member.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()));

        let mut blank = m.add_request();
        write_uuid(blank.get().init_group(), Some(a));
        blank.get().init_member().set_person("  ");
        let e = blank.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()));

        let mut direct = m.direct_request();
        write_uuid(direct.get().init_group(), Some(unknown()));
        let e = direct.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (404, gp("not-found")));

        let mut remove = m.remove_request();
        write_uuid(remove.get().init_membership(), Some(unknown()));
        let e = remove.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (404, gp("not-found")));
    })
    .await;
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
