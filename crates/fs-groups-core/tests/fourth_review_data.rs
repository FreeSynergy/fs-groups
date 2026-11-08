//! Text shapes, and values edited by hand (fourth review).

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{Agents, FakeDirectory, ME, person, setup};
use fs_groups_core::{Group, GroupRepo, Member, Mirror, Right, RightFlow};
use fs_objects::{ObjectId, Store};

const BOT: &str = "https://example.org/agents/bot";

async fn world(agents: &[&str]) -> (GroupRepo, Arc<dyn Store>, Arc<Agents>) {
    let (r, store) = setup().await;
    let list = Agents::with(agents);
    let r = r
        .with_ai_marking(list.clone())
        .with_admin_roles(["owner"])
        .unwrap();
    (r, store, list)
}

async fn mk(r: &GroupRepo, name: &str, flow: RightFlow) -> Group {
    let g = r.create(name, "t").await.unwrap();
    r.set_right_flow(g.id, flow).await.unwrap()
}

async fn put(r: &GroupRepo, g: &Group, who: &str) {
    let who = if who.starts_with("http") {
        who.to_string()
    } else {
        person(who)
    };
    r.add(g.id, Member::Person(who), "").await.unwrap();
}

#[tokio::test]
async fn should_6_and_7_text_shapes() {
    let (r, _, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    for bad in [
        "a\u{3164}b",
        "a\u{115f}b",
        "a\u{1160}b",
        "a\u{ffa0}b",
        "ab\u{200c}",
        "a\u{200b}b",
    ] {
        assert!(
            r.add(g.id, Member::Person(bad.into()), "").await.is_err(),
            "{bad:?}"
        );
    }
    for good in [
        "https://example.org/people/mu\u{308}ller",
        "https://example.org/people/\u{915}\u{94d}\u{937}",
        "https://example.org/people/\u{645}\u{6cc}\u{200c}\u{62e}\u{648}\u{627}\u{647}\u{645}",
        "a^b{c|d}`e'f\"g@example.org",
    ] {
        r.add(g.id, Member::Person(good.into()), "")
            .await
            .unwrap_or_else(|e| panic!("{good:?}: {e}"));
    }
    // decomposed and composed are one person
    let m = r
        .add(
            g.id,
            Member::Person("https://example.org/people/m\u{fc}ller".into()),
            "",
        )
        .await
        .unwrap();
    assert_eq!(
        m.member,
        Member::Person("https://example.org/people/m\u{fc}ller".into())
    );
    assert_eq!(r.direct(g.id).await.unwrap().len(), 4);
}

#[tokio::test]
async fn should_8_an_edited_value_that_disagrees_with_the_content_is_unreadable() {
    let (r, store, _) = world(&[BOT]).await;
    let h = mk(&r, "h", RightFlow::None).await;
    put(&r, &h, "ada").await;
    let target = r.create("target", "t").await.unwrap();
    let d = r
        .grant(target.id, Member::Group(h.id), Right::Manage)
        .await
        .unwrap();
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    // the index says read, the content says manage
    let mut o = store.get(d.id).await.unwrap().unwrap();
    o.index.insert("right".into(), vec!["read".into()]);
    store.put(o, ME).await.unwrap();
    put(&r, &h, BOT).await;
    let report = r.sync_to(&dir).await.unwrap();
    assert!(!dir.get(h.id).unwrap().members.contains(&BOT.to_string()));
    assert_eq!(report.unreadable, [d.id]);
}

#[tokio::test]
async fn should_8_a_membership_of_a_changed_type_or_with_a_flipped_member_is_reported() {
    let (r, store, _) = world(&[]).await;
    let g = mk(&r, "g", RightFlow::None).await;
    let (a, b) = (
        r.add(g.id, Member::Person(person("ada")), "")
            .await
            .unwrap(),
        r.add(g.id, Member::Person(person("bo")), "").await.unwrap(),
    );
    let dir = FakeDirectory::default();
    r.sync_to(&dir).await.unwrap();
    let mut obj = store.get(a.id).await.unwrap().unwrap();
    obj.envelope.object_type = "Note".into();
    store.put(obj, ME).await.unwrap();
    let mut obj = store.get(b.id).await.unwrap().unwrap();
    obj.index.insert("member_kind".into(), vec!["group".into()]);
    obj.index
        .insert("member".into(), vec![ObjectId::new().to_string()]);
    store.put(obj, ME).await.unwrap();
    let report = r.sync_to(&dir).await.unwrap();
    let want: BTreeSet<_> = [a.id, b.id].into();
    assert_eq!(
        report.unreadable.iter().copied().collect::<BTreeSet<_>>(),
        want
    );
    assert_eq!(
        dir.get(g.id).unwrap().members.len(),
        2,
        "nobody is taken out"
    );
}
