//! Group items: create, rename, kind, get, list, delete.
//!
//! Pattern: Repository — every write goes through a `Writer` (the write lock), reads included in the
//! same step; `delete` is a Command-like cascade that removes everything that hangs on the group,
//! then the group (children stay).

use fs_store::{ObjectId, Order, Query};

use super::consts::{CONTEXT, GROUP_TYPE};
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::model::{Cursor, Group, Member, Page, RightFlow};

impl GroupRepo {
    /// `group.create`: a new group with no parent. The name must not be blank.
    pub async fn create(&self, name: &str, kind: &str) -> Result<Group> {
        if name.trim().is_empty() {
            return Err(Error::Conflict("a group needs a name".into()));
        }
        let g = self.new_group(name.trim(), kind.trim());
        self.writer().await.put(self.group_object(&g, None)).await?;
        Ok(g)
    }

    /// `group.read`: one group, if it exists.
    pub async fn get(&self, id: ObjectId) -> Result<Option<Group>> {
        let Some(o) = self.load(id).await? else {
            return Ok(None);
        };
        if o.envelope.object_type != GROUP_TYPE || o.envelope.context.as_deref() != Some(CONTEXT) {
            return Ok(None);
        }
        Ok(Group::from_content(id, &o.content))
    }

    pub(super) async fn require(&self, id: ObjectId) -> Result<Group> {
        self.get(id)
            .await?
            .ok_or(Error::NotFound { what: "group", id })
    }

    /// `group.update`: change the name.
    pub async fn rename(&self, id: ObjectId, name: &str) -> Result<Group> {
        if name.trim().is_empty() {
            return Err(Error::Conflict("a group needs a name".into()));
        }
        let w = self.writer().await;
        let mut g = w.require(id).await?;
        g.name = name.trim().into();
        w.save(&g).await?;
        Ok(g)
    }

    /// `group.update`: change the kind.
    pub async fn set_kind(&self, id: ObjectId, kind: &str) -> Result<Group> {
        let w = self.writer().await;
        let mut g = w.require(id).await?;
        g.kind = kind.trim().into();
        w.save(&g).await?;
        Ok(g)
    }

    /// `group.update`: set whether and where the group passes its rights on.
    pub async fn set_right_flow(&self, id: ObjectId, flow: RightFlow) -> Result<Group> {
        let w = self.writer().await;
        let mut g = w.require(id).await?;
        g.right_flow = flow;
        w.save(&g).await?;
        Ok(g)
    }

    /// `group.read`: all groups in name order, whole (no paging).
    pub(super) async fn all_groups(&self) -> Result<Vec<Group>> {
        Ok(self.read_groups().await?.0)
    }

    /// All groups in name order, and the ids of stored groups that cannot be read as a group.
    pub(super) async fn read_groups(&self) -> Result<(Vec<Group>, Vec<ObjectId>)> {
        let q = Query::new()
            .of_type(GROUP_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::NameAsc);
        let found = self.reads.query(&q).await?;
        let (mut groups, mut unreadable) = (vec![], vec![]);
        for o in &found {
            match Group::from_content(o.envelope.id, &o.content) {
                Some(g) => groups.push(g),
                None => unreadable.push(o.envelope.id),
            }
        }
        Ok((groups, unreadable))
    }

    /// `group.read`: groups in name order, one page.
    pub async fn list(&self, cursor: Option<&Cursor>) -> Result<Page<Group>> {
        let offset = cursor.map_or(0, Cursor::offset);
        let mut q = Query::new()
            .of_type(GROUP_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::NameAsc);
        q.offset = offset;
        q.limit = Some(self.page_size + 1);
        let found = self.reads.query(&q).await?;
        let more = found.len() > self.page_size;
        let items = found
            .iter()
            .take(self.page_size)
            .filter_map(|o| Group::from_content(o.envelope.id, &o.content))
            .collect();
        Ok(Page {
            items,
            next: more.then(|| Cursor::at(offset + self.page_size)),
        })
    }

    /// `group.delete`: remove the group with its memberships, invitations, delegations (also the ones
    /// it holds) and its links to parents and children. Children stay.
    pub async fn delete(&self, id: ObjectId) -> Result<()> {
        let w = self.writer().await;
        w.require(id).await?;
        let mut depending = vec![];
        for key in ["group", "child", "parent_of"] {
            let q = Query::new()
                .in_context(CONTEXT)
                .with_index(key, id.to_string());
            depending.extend(w.reads.query(&q).await?);
        }
        let held = Query::new()
            .in_context(CONTEXT)
            .with_index("holder_kind", Member::Group(id).kind())
            .with_index("holder", id.to_string());
        depending.extend(w.reads.query(&held).await?);
        for o in depending {
            w.remove_object(o.envelope.id).await?;
        }
        for mut child in w
            .all_groups()
            .await?
            .into_iter()
            .filter(|g| g.parents.contains(&id))
        {
            child.parents.retain(|p| *p != id);
            w.save(&child).await?;
        }
        w.remove_object(id).await?;
        Ok(())
    }
}

impl Writer<'_> {
    /// Write an existing group back, keeping its envelope.
    pub(super) async fn save(&self, g: &Group) -> Result<()> {
        let existing = self.load(g.id).await?.map(|o| o.envelope);
        self.put(self.group_object(g, existing)).await
    }
}
