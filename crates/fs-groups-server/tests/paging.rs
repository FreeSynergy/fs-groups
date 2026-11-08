//! `Items.list`: pages by count and by weight, the cursor as a consumer sees it.

mod common;

use common::{create, items_of, problem, with_server};
use fs_interfaces::groups_capnp::items;
use fs_interfaces::rpc::INVALID_REQUEST;
use fs_interfaces::rpc::{MAX_PAGE_BYTES, read_envelope};

/// All pages: the names on each.
async fn pages(items: &items::Client) -> Vec<Vec<String>> {
    let mut out = vec![];
    let mut cursor: Vec<u8> = vec![];
    loop {
        let mut list = items.list_request();
        list.get().init_cursor().set_opaque(&cursor);
        let answer = list.send().promise.await.unwrap();
        let page = answer.get().unwrap().get_page().unwrap();
        let names = page
            .get_items()
            .unwrap()
            .iter()
            .map(|g| read_envelope(g.get_envelope().unwrap()).unwrap().name);
        out.push(names.collect());
        cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
        if cursor.is_empty() {
            return out;
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lists_more_than_a_core_page_in_name_order() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for n in (0..230).rev() {
            create(&items, &format!("group {n:03}")).await;
        }
        let pages = pages(&items).await;
        let sizes: Vec<_> = pages.iter().map(Vec::len).collect();
        assert_eq!(sizes, [100, 100, 30]);
        let want: Vec<_> = (0..230).map(|n| format!("group {n:03}")).collect();
        assert_eq!(
            pages.concat(),
            want,
            "every group once, in name order, across pages"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_page_ends_by_weight_before_the_message_limit() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let big = "x".repeat(1024 * 1024 - 8);
        let count = 2 * MAX_PAGE_BYTES / (2 * big.len());
        for n in 0..count {
            create(&items, &format!("{n:03}{big}")).await;
        }
        let pages = pages(&items).await;
        assert!(pages.len() >= 2, "cut by weight, not by count");
        assert!(pages[0].len() < 100);
        assert_eq!(pages.iter().map(Vec::len).sum::<usize>(), count);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn groups_of_one_name_all_come_once() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut made = vec![];
        for _ in 0..120 {
            made.push(create(&items, "Same").await);
        }
        let mut seen = vec![];
        let mut cursor: Vec<u8> = vec![];
        loop {
            let mut list = items.list_request();
            list.get().init_cursor().set_opaque(&cursor);
            let answer = list.send().promise.await.unwrap();
            let page = answer.get().unwrap().get_page().unwrap();
            for g in page.get_items().unwrap() {
                seen.push(
                    read_envelope(g.get_envelope().unwrap())
                        .unwrap()
                        .id
                        .as_uuid(),
                );
            }
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        seen.sort();
        made.sort();
        assert_eq!(seen, made);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_nobody_handed_out_is_refused() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut list = items.list_request();
        list.get().init_cursor().set_opaque(b"not a cursor");
        let e = list.send().promise.await.err().unwrap();
        assert_eq!(problem(&e), (422, INVALID_REQUEST.to_string()));
    })
    .await;
}
