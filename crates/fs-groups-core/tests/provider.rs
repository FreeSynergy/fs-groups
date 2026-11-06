//! The provider rule (ADR 0045): every provider has every method; what it does not offer says so.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::repo;
use fs_groups_core::{
    Cursor, Delegations, DirectoryGroup, Error, Group, GroupDirectory, Groups, Invitations, Member,
    Memberships, Mirror, Page, Result, Right,
};
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

/// A provider that can only read, for example a mirror of someone else's groups.
struct ReadOnly;

#[async_trait]
impl Groups for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("read-only");
        c.supported("group.read")
            .unsupported("group.create", "this source cannot be written");
        c
    }
    async fn get(&self, _: ObjectId) -> Result<Option<Group>> {
        Ok(None)
    }
    async fn list(&self, _: Option<&Cursor>) -> Result<Page<Group>> {
        Ok(Page {
            items: vec![],
            next: None,
        })
    }
    async fn parents(&self, _: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(vec![])
    }
    async fn children(&self, _: ObjectId) -> Result<Vec<ObjectId>> {
        Ok(vec![])
    }
}
#[async_trait]
impl Memberships for ReadOnly {
    async fn direct(&self, _: ObjectId) -> Result<Vec<fs_groups_core::Membership>> {
        Ok(vec![])
    }
}
#[async_trait]
impl Invitations for ReadOnly {
    async fn list(&self, _: ObjectId) -> Result<Vec<fs_groups_core::Invitation>> {
        Ok(vec![])
    }
}
#[async_trait]
impl Delegations for ReadOnly {
    async fn list(&self, _: ObjectId) -> Result<Vec<fs_groups_core::Delegation>> {
        Ok(vec![])
    }
}

fn not_possible<T: std::fmt::Debug>(r: Result<T>, capability: &str) {
    let Err(Error::Unsupported(u)) = r else {
        panic!("expected an explicit not possible for {capability}")
    };
    assert_eq!((u.capability(), u.status()), (capability, 501));
}

#[tokio::test]
async fn a_missing_method_says_not_possible_with_its_capability_id() {
    let g: Arc<dyn Groups> = Arc::new(ReadOnly);
    let id = ObjectId::new();
    not_possible(g.create("x", "y").await, "group.create");
    not_possible(g.rename(id, "x").await, "group.update");
    not_possible(g.delete(id).await, "group.delete");
    not_possible(g.request_parent(id, id).await, "group.nest");
    assert!(
        g.ancestors(id).await.unwrap().is_empty(),
        "provided in terms of parents"
    );
    assert!(!g.capabilities().is_supported("group.create"));

    let m: Arc<dyn Memberships> = Arc::new(ReadOnly);
    not_possible(
        m.add(id, Member::Person("p".into()), "").await,
        "member.add",
    );
    not_possible(m.remove(id).await, "member.remove");
    not_possible(m.effective(id).await, "member.read");
    let i: Arc<dyn Invitations> = Arc::new(ReadOnly);
    not_possible(i.invite(id, "a", "", None).await, "invite.create");
    not_possible(i.decline(id).await, "invite.answer");
    let d: Arc<dyn Delegations> = Arc::new(ReadOnly);
    not_possible(
        d.grant(id, Member::Person("p".into()), Right::Read).await,
        "right.grant",
    );
    not_possible(d.may_do(id, "p", Right::Read).await, "right.read");
}

#[tokio::test]
async fn the_store_backed_provider_reports_what_it_can_and_what_not() {
    let g: Arc<dyn Groups> = Arc::new(repo().await);
    let caps = g.capabilities();
    for id in [
        "group.create",
        "group.update",
        "group.delete",
        "group.read",
        "group.nest",
        "member.add",
        "member.remove",
        "member.read",
        "invite.create",
        "invite.answer",
        "invite.read",
        "right.grant",
        "right.read",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
    let no = caps.why_not("group.subscribe").unwrap();
    assert_eq!(no.reason(), "not built yet; it is possible");
}

#[tokio::test]
async fn a_directory_that_cannot_apply_is_refused_plainly() {
    struct ReadOnlyDir;
    #[async_trait::async_trait]
    impl GroupDirectory for ReadOnlyDir {
        fn capabilities(&self) -> fs_interfaces::Capabilities {
            let mut c = fs_interfaces::Capabilities::new("ro");
            c.supported("directory.read")
                .unsupported("directory.apply", "read only");
            c
        }
        async fn read(
            &self,
            _: fs_store::ObjectId,
        ) -> fs_groups_core::Result<Option<DirectoryGroup>> {
            Ok(None)
        }
        async fn list(&self) -> fs_groups_core::Result<Vec<DirectoryGroup>> {
            Ok(vec![])
        }
    }
    let r = repo().await;
    assert!(matches!(
        r.sync_to(&ReadOnlyDir).await,
        Err(fs_groups_core::Error::Unsupported(_))
    ));
    let err = ReadOnlyDir
        .apply(&DirectoryGroup {
            id: fs_store::ObjectId::new(),
            name: String::new(),
            members: vec![],
            nested: vec![],
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, fs_groups_core::Error::Unsupported(u) if u.capability() == "directory.apply")
    );
}
