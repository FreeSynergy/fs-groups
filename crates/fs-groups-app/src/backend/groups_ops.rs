//! The commands on groups and on requests to become a child (`Items` of the contract).
//!
//! Pattern: Command (one function per command) over a Proxy — each function builds one request, sends it on the
//! worker and reads the answer into frontend values. No rule lives here: the core decides (cycles, consent).

use fs_admin_frontend::{Outcome, Record, Values};
use fs_interfaces::groups_capnp::RightFlow;
use fs_interfaces::rpc::write_uuid;
use fs_types::{Edit, FieldEdit};
use uuid::Uuid;

use super::input::{self, flow};
use super::read;
use crate::remote::{Clients, GroupsRemote, RunError};

/// Every group, paged to the end.
pub async fn all_groups(c: &Clients) -> Result<Vec<Record>, RunError> {
    let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
    loop {
        let mut request = c.items.list_request();
        request.get().init_cursor().set_opaque(&cursor);
        let answer = request.send().promise.await?;
        let page = answer.get()?.get_page()?;
        for g in page.get_items()? {
            found.push(read::group(g)?);
        }
        let next = page.get_next()?.get_opaque()?;
        if next.is_empty() {
            return Ok(found);
        }
        cursor = next.to_vec();
    }
}

fn matches(record: &Record, term: &str) -> bool {
    let term = term.to_lowercase();
    term.is_empty()
        || ["name", "kind"].iter().any(|f| {
            record
                .values
                .get(*f)
                .is_some_and(|v| v.iter().any(|s| s.to_lowercase().contains(&term)))
        })
}

/// `find`: the groups whose name or kind holds the term; an empty term lists all.
pub fn find(remote: &GroupsRemote, query: &Values) -> Result<Vec<Record>, RunError> {
    let term = input::one(query, "term").unwrap_or_default().to_owned();
    let all = remote.run(|c| async move { all_groups(&c).await })?;
    Ok(all.into_iter().filter(|g| matches(g, &term)).collect())
}

/// `create`.
pub fn create(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let name = input::need(v, "name")?.to_owned();
    let kind = input::one(v, "kind").unwrap_or_default().to_owned();
    remote.run(move |c| async move {
        let mut request = c.items.create_request();
        request.get().set_name(name.as_str());
        request.get().set_kind(kind.as_str());
        let answer = request.send().promise.await?;
        let g = read::group(answer.get()?.get_group()?)?;
        Ok(Outcome {
            id: Some(g.id),
            values: Values::new(),
        })
    })
}

/// `delete`.
pub fn delete(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        let mut request = c.items.delete_request();
        write_uuid(request.get().init_id(), Some(id));
        request.send().promise.await?;
        Ok(Outcome::default())
    })
}

/// What the three calls behind `change` have to do for one edit.
#[derive(Debug, PartialEq, Eq)]
pub struct GroupChange {
    /// New name.
    pub name: Option<String>,
    /// New kind (empty = cleared).
    pub kind: Option<String>,
    /// New right flow.
    pub flow: Option<RightFlow>,
}

/// Map the fields of an `Edit` to the calls of the contract.
///
/// # Errors
/// [`RunError::Bad`] for a name that is cleared (a group needs a name), a flow that is no flow, or a field that is
/// not editable here (`parents` change through the nesting commands).
pub fn plan_change(edit: &Edit) -> Result<GroupChange, RunError> {
    let bad = |f: &str| RunError::Bad { field: f.to_owned() };
    let mut plan = GroupChange { name: None, kind: None, flow: None };
    for FieldEdit { path, values, .. } in &edit.fields {
        let first = values.first().cloned();
        match path.as_slice() {
            [n] if n == "name" => plan.name = Some(first.ok_or_else(|| bad("name"))?),
            [n] if n == "kind" => plan.kind = Some(first.unwrap_or_default()),
            [n] if n == "rightFlow" => {
                plan.flow = Some(first.map_or(Ok(RightFlow::None), |k| flow(&k, "rightFlow"))?);
            }
            [n, ..] => return Err(bad(n)),
            [] => return Err(bad("")),
        }
    }
    Ok(plan)
}

/// `change`: an `Edit` of the group.
pub fn change(remote: &GroupsRemote, record: &str, edit: &Edit) -> Result<Outcome, RunError> {
    let id = Uuid::parse_str(record).map_err(|_| RunError::NotFound)?;
    let plan = plan_change(edit)?;
    remote.run(move |c| async move {
        if let Some(name) = plan.name {
            let mut r = c.items.rename_request();
            write_uuid(r.get().init_id(), Some(id));
            r.get().set_name(name.as_str());
            r.send().promise.await?;
        }
        if let Some(kind) = plan.kind {
            let mut r = c.items.set_kind_request();
            write_uuid(r.get().init_id(), Some(id));
            r.get().set_kind(kind.as_str());
            r.send().promise.await?;
        }
        if let Some(flow) = plan.flow {
            let mut r = c.items.set_right_flow_request();
            write_uuid(r.get().init_id(), Some(id));
            r.get().set_flow(flow);
            r.send().promise.await?;
        }
        Ok(Outcome {
            id: Some(id.to_string()),
            values: Values::new(),
        })
    })
}

/// `parents`, `children`, `ancestors`, `descendants`: the ids of related groups.
pub fn related(remote: &GroupsRemote, v: &Values, which: &'static str) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    let ids = remote.run(move |c| async move {
        macro_rules! ask {
            ($req:ident) => {{
                let mut r = c.items.$req();
                write_uuid(r.get().init_id(), Some(id));
                let answer = r.send().promise.await?;
                read::id_list(answer.get()?.get_ids())?
            }};
        }
        Ok(match which {
            "parents" => ask!(parents_request),
            "children" => ask!(children_request),
            "ancestors" => ask!(ancestors_request),
            _ => ask!(descendants_request),
        })
    })?;
    let mut values = Values::new();
    values.insert("ids".into(), ids);
    Ok(Outcome { id: None, values })
}

/// `removeParent`.
pub fn remove_parent(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let (id, parent) = (input::id(v, "id")?, input::id(v, "parent")?);
    remote.run(move |c| async move {
        let mut r = c.items.remove_parent_request();
        write_uuid(r.get().init_id(), Some(id));
        write_uuid(r.get().init_parent(), Some(parent));
        r.send().promise.await?;
        Ok(Outcome::default())
    })
}

/// `ParentRequest.create`.
pub fn request_parent(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let (child, parent) = (input::id(v, "child")?, input::id(v, "parent")?);
    remote.run(move |c| async move {
        let mut r = c.items.request_parent_request();
        write_uuid(r.get().init_child(), Some(child));
        write_uuid(r.get().init_parent(), Some(parent));
        let answer = r.send().promise.await?;
        let req = read::parent_request(answer.get()?.get_request()?)?;
        Ok(Outcome {
            id: Some(req.id),
            values: Values::new(),
        })
    })
}

/// `ParentRequest.find`: the requests to one parent, or to every group when none is named.
pub fn parent_requests(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let parent = input::id_opt(v, "parent")?;
    remote.run(move |c| async move {
        let parents = match parent {
            Some(p) => vec![p],
            None => all_groups(&c)
                .await?
                .iter()
                .filter_map(|g| Uuid::parse_str(&g.id).ok())
                .collect(),
        };
        let mut found = Vec::new();
        for p in parents {
            let mut r = c.items.parent_requests_request();
            write_uuid(r.get().init_parent(), Some(p));
            let answer = r.send().promise.await?;
            for item in answer.get()?.get_items()? {
                found.push(read::parent_request(item)?);
            }
        }
        Ok(found)
    })
}

/// `approve` and `decline` of a request.
pub fn answer_request(remote: &GroupsRemote, v: &Values, approve: bool) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        if approve {
            let mut r = c.items.approve_parent_request();
            write_uuid(r.get().init_request(), Some(id));
            let answer = r.send().promise.await?;
            let g = read::group(answer.get()?.get_group()?)?;
            let mut values = Values::new();
            values.insert("group".into(), vec![g.id]);
            Ok(Outcome { id: None, values })
        } else {
            let mut r = c.items.decline_parent_request();
            write_uuid(r.get().init_request(), Some(id));
            r.send().promise.await?;
            Ok(Outcome::default())
        }
    })
}
