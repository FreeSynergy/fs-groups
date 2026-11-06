//! Parent requests: a group asks to nest under a parent, the parent's side approves or declines.
//!
//! Pattern: Repository + State — **provisional**: who consents to a link is still the owner's to
//! answer. `request_parent` only writes a request; the link `fs:parents` exists once
//! `approve_parent` ran, and the mirror only ever sees approved links. The cycle rule is checked when
//! asking and again when approving, under the write lock.
//!
//! What is and is not enforced: the approver is the repository's `actor` — whatever string the
//! caller built the `GroupRepo` with. Nothing authenticates it here; the server in front must do
//! that. It may approve if it holds `manage` on the parent by `may_do`, or, while the parent has no
//! delegation at all, if it is the one who created the parent. `grant` itself is not guarded yet,
//! so anybody who can call it can give themselves `manage`.

use fs_store::{ObjectId, Order, Query, StoredObject};
use serde_json::json;

use super::codec;
use super::consts::{AS2_TYPE, CONTEXT, PARENT_REQUEST_TYPE};
use super::envelope::context;
use super::handle::{GroupRepo, Writer};
use crate::error::{Error, Result};
use crate::model::{Group, ParentRequest, RequestState, Right};

impl GroupRepo {
    /// `group.nest`: ask for `parent` to become a parent of `child`. Nothing is linked yet. Refused
    /// when it would close a loop, or when the link exists. Asking again while a request is pending
    /// returns that request.
    pub async fn request_parent(&self, child: ObjectId, parent: ObjectId) -> Result<ParentRequest> {
        let w = self.writer().await;
        let g = w.require(child).await?;
        w.refuse_cycle(child, parent).await?;
        if g.parents.contains(&parent) {
            return Err(Error::Conflict(format!(
                "{parent} is already a parent of {child}"
            )));
        }
        if let Some(same) = w
            .requests_where(&[("child", child), ("parent_of", parent)])
            .await?
            .into_iter()
            .find(|r| r.state == RequestState::Pending)
        {
            return Ok(same);
        }
        let env = w.fresh_envelope(
            PARENT_REQUEST_TYPE,
            &format!("{child} asks {parent} to be its parent"),
        );
        let content = json!({
            "@context": context(), "type": PARENT_REQUEST_TYPE, "fs:child": child.to_string(),
            "fs:parent": parent.to_string(), "fs:state": RequestState::Pending.as_str(),
        });
        let obj = StoredObject::new(env, AS2_TYPE, content)
            .with_index("child", child.to_string())
            .with_index("parent_of", parent.to_string())
            .with_index("state", RequestState::Pending.as_str());
        let id = obj.envelope.id;
        w.put(obj).await?;
        Ok(ParentRequest {
            id,
            child,
            parent,
            requester: self.actor.clone(),
            state: RequestState::Pending,
        })
    }

    /// `group.nest`: approve on behalf of the parent. The link is made (the loop rule is checked
    /// again). Refused with [`Error::Forbidden`] unless the actor may decide for the parent (see the
    /// module notes). Returns the child with its new parent.
    pub async fn approve_parent(&self, request: ObjectId) -> Result<Group> {
        let w = self.writer().await;
        let (o, r) = w.pending_request(request).await?;
        w.may_decide_for(r.parent).await?;
        let mut child = w.require(r.child).await?;
        w.refuse_cycle(r.child, r.parent).await?;
        if !child.parents.contains(&r.parent) {
            child.parents.push(r.parent);
            w.save(&child).await?;
        }
        w.set_request_state(o, RequestState::Approved).await?;
        Ok(child)
    }

    /// `group.nest`: decline on behalf of the parent. Same rule as [`GroupRepo::approve_parent`].
    pub async fn decline_parent(&self, request: ObjectId) -> Result<()> {
        let w = self.writer().await;
        let (o, r) = w.pending_request(request).await?;
        w.may_decide_for(r.parent).await?;
        w.set_request_state(o, RequestState::Declined).await
    }

    /// `group.nest`: the requests made to `parent`, in any state, oldest first.
    pub async fn parent_requests(&self, parent: ObjectId) -> Result<Vec<ParentRequest>> {
        self.require(parent).await?;
        self.requests_where(&[("parent_of", parent)]).await
    }

    async fn requests_where(&self, keys: &[(&str, ObjectId)]) -> Result<Vec<ParentRequest>> {
        let mut q = Query::new()
            .of_type(PARENT_REQUEST_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::PublishedAsc);
        for (k, v) in keys {
            q = q.with_index(*k, v.to_string());
        }
        Ok(self
            .reads
            .query(&q)
            .await?
            .iter()
            .filter_map(codec::parent_request)
            .collect())
    }
}

impl Writer<'_> {
    async fn pending_request(&self, id: ObjectId) -> Result<(StoredObject, ParentRequest)> {
        let (o, r) = self
            .load(id)
            .await?
            .and_then(|o| codec::parent_request(&o).map(|r| (o, r)))
            .ok_or(Error::NotFound {
                what: "parent request",
                id,
            })?;
        if r.state == RequestState::Pending {
            Ok((o, r))
        } else {
            Err(Error::Conflict(format!(
                "request {id} is already {}",
                r.state.as_str()
            )))
        }
    }

    async fn set_request_state(&self, mut o: StoredObject, state: RequestState) -> Result<()> {
        o.index.insert("state".into(), vec![state.as_str().into()]);
        o.content["fs:state"] = json!(state.as_str());
        self.put(o).await
    }

    /// Both groups exist and `parent` is not `child` or one of its descendants.
    async fn refuse_cycle(&self, child: ObjectId, parent: ObjectId) -> Result<()> {
        let graph = self.known_graph(child).await?;
        if !graph.contains(parent) {
            return Err(Error::NotFound {
                what: "group",
                id: parent,
            });
        }
        match graph.path_up(parent, child) {
            Some(path) => Err(Error::Cycle {
                group: child,
                parent,
                path,
            }),
            None => Ok(()),
        }
    }

    /// Whether the actor may answer for `parent`: nobody holds any delegation on `parent` yet and the
    /// actor created it, or it holds `manage` there (`may_do`, which loads the whole graph, so the
    /// cheap case is looked at first).
    async fn may_decide_for(&self, parent: ObjectId) -> Result<()> {
        let no_delegation = self.delegations(parent).await?.is_empty();
        let created_by_actor = self
            .load(parent)
            .await?
            .is_some_and(|o| o.envelope.attributed_to.as_deref() == Some(self.actor.as_str()));
        // an agent never answers for a group (ADR 0056), as creator or otherwise
        let human = self.is_known_human(&self.actor).await?;
        if human
            && ((no_delegation && created_by_actor)
                || self.may_do(parent, &self.actor, Right::Manage).await?)
        {
            Ok(())
        } else {
            Err(Error::Forbidden(format!(
                "{} may not answer for group {parent}",
                self.actor
            )))
        }
    }
}
