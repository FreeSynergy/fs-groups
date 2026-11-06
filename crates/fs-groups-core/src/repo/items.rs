//! Group items: create, rename, kind, get, list, delete.
//!
//! Pattern: Repository — every write is one object put; `delete` is a Command-like cascade that
//! removes everything that hangs on the group, then the group (children stay).

use fs_store::{ObjectId, Order, Query};

use super::consts::{CONTEXT, GROUP_TYPE};
use super::handle::GroupRepo;
use crate::error::{Error, Result};
use crate::model::{Cursor, Group, Member, Page, RightFlow};

impl GroupRepo {
    /// `group.create`: a new group with no parent. The name must not be blank.
    pub async fn create(&self, name: &str, kind: &str) -> Result<Group> {
        if name.trim().is_empty() {
            return Err(Error::Conflict("a group needs a name".into()));
        }
        let g = self.new_group(name.trim(), kind.trim());
        self.objects
            .put(self.group_object(&g, None), &self.actor)
            .await?;
        Ok(g)
    }

    /// `group.read`: one group, if it exists.
    pub async fn get(&self, id: ObjectId) -> Result<Option<Group>> {
        let Some(o) = self.objects.get(id).await? else {
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
        let mut g = self.require(id).await?;
        g.name = name.trim().into();
        self.save(&g).await?;
        Ok(g)
    }

    /// `group.update`: change the kind.
    pub async fn set_kind(&self, id: ObjectId, kind: &str) -> Result<Group> {
        let mut g = self.require(id).await?;
        g.kind = kind.trim().into();
        self.save(&g).await?;
        Ok(g)
    }

    /// `group.update`: set whether and where the group passes its rights on.
    pub async fn set_right_flow(&self, id: ObjectId, flow: RightFlow) -> Result<Group> {
        let mut g = self.require(id).await?;
        g.right_flow = flow;
        self.save(&g).await?;
        Ok(g)
    }

    /// Write an existing group back, keeping its envelope.
    pub(super) async fn save(&self, g: &Group) -> Result<()> {
        let existing = self.objects.get(g.id).await?.map(|o| o.envelope);
        self.objects
            .put(self.group_object(g, existing), &self.actor)
            .await?;
        Ok(())
    }

    /// `group.read`: all groups in name order, whole (no paging).
    pub(super) async fn all_groups(&self) -> Result<Vec<Group>> {
        let q = Query::new()
            .of_type(GROUP_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::NameAsc);
        let found = self.reads.query(&q).await?;
        Ok(found
            .iter()
            .filter_map(|o| Group::from_content(o.envelope.id, &o.content))
            .collect())
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
        let _write = self.lock.lock().await;
        self.require(id).await?;
        let mut depending = self
            .reads
            .query(
                &Query::new()
                    .in_context(CONTEXT)
                    .with_index("group", id.to_string()),
            )
            .await?;
        let held = Query::new()
            .in_context(CONTEXT)
            .with_index("holder_kind", Member::Group(id).kind())
            .with_index("holder", id.to_string());
        depending.extend(self.reads.query(&held).await?);
        for o in depending {
            self.objects.delete(o.envelope.id, &self.actor).await?;
        }
        for mut child in self
            .all_groups()
            .await?
            .into_iter()
            .filter(|g| g.parents.contains(&id))
        {
            child.parents.retain(|p| *p != id);
            self.save(&child).await?;
        }
        self.objects.delete(id, &self.actor).await?;
        Ok(())
    }
}
