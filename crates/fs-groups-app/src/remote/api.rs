//! `GroupsApi` for `GroupsRemote`: one request per method.
//!
//! Pattern: Proxy — each method is a job run on the worker thread.

use fs_interfaces::common_capnp;
use fs_interfaces::groups_capnp::items;
use fs_interfaces::rpc::{read_uuid, write_uuid};

use super::convert::{parse_id, problem, read_group};
use super::model::{ApiError, Group, GroupsApi, Member};
use super::proxy::GroupsRemote;

impl GroupsApi for GroupsRemote {
    fn list(&self) -> Result<Vec<Group>, ApiError> {
        self.run(|c| async move {
            let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
            loop {
                let mut request = c.items.list_request();
                request.get().init_cursor().set_opaque(&cursor);
                let answer = request.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(items::list_results::Reader::get_page)
                    .map_err(|e| problem(&e))?;
                for g in page.get_items().map_err(|e| problem(&e))? {
                    found.push(read_group(g).map_err(|e| problem(&e))?);
                }
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    return Ok(found);
                }
                cursor = next.to_vec();
            }
        })
    }

    fn get(&self, id: &str) -> Result<Option<Group>, ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let mut request = c.items.get_request();
            write_uuid(request.get().init_id(), Some(id));
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            let r = answer.get().map_err(|e| problem(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            let g = r
                .get_group()
                .and_then(read_group)
                .map_err(|e| problem(&e))?;
            Ok(Some(g))
        })
    }

    fn create(&self, name: &str, kind: &str) -> Result<Group, ApiError> {
        let (name, kind) = (name.to_string(), kind.to_string());
        self.run(move |c| async move {
            let mut request = c.items.create_request();
            request.get().set_name(name.as_str());
            request.get().set_kind(kind.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::create_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn rename(&self, id: &str, name: &str) -> Result<Group, ApiError> {
        let (id, name) = (parse_id(id)?, name.to_string());
        self.run(move |c| async move {
            let mut request = c.items.rename_request();
            write_uuid(request.get().init_id(), Some(id));
            request.get().set_name(name.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::rename_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn set_kind(&self, id: &str, kind: &str) -> Result<Group, ApiError> {
        let (id, kind) = (parse_id(id)?, kind.to_string());
        self.run(move |c| async move {
            let mut request = c.items.set_kind_request();
            write_uuid(request.get().init_id(), Some(id));
            request.get().set_kind(kind.as_str());
            let answer = request.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(items::set_kind_results::Reader::get_group)
                .and_then(read_group)
                .map_err(|e| problem(&e))
        })
    }

    fn delete(&self, id: &str) -> Result<(), ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let mut request = c.items.delete_request();
            write_uuid(request.get().init_id(), Some(id));
            request
                .send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    fn members(&self, id: &str) -> Result<Vec<Member>, ApiError> {
        let id = parse_id(id)?;
        self.run(move |c| async move {
            let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
            loop {
                let mut request = c.memberships.direct_request();
                write_uuid(request.get().init_group(), Some(id));
                request.get().init_cursor().set_opaque(&cursor);
                let answer = request.send().promise.await.map_err(|e| problem(&e))?;
                let r = answer.get().map_err(|e| problem(&e))?;
                for m in r.get_items().map_err(|e| problem(&e))? {
                    let who = m.get_member().map_err(|e| problem(&e))?;
                    let (who, is_group) =
                        match who.which().map_err(|e| ApiError::Failed(e.to_string()))? {
                            fs_interfaces::groups_capnp::member::Which::Person(p) => (
                                p.map_err(|e| problem(&e))?
                                    .to_string()
                                    .map_err(|e| ApiError::Failed(e.to_string()))?,
                                false,
                            ),
                            fs_interfaces::groups_capnp::member::Which::Group(g) => (
                                read_uuid(g.map_err(|e| problem(&e))?)
                                    .map(|u| u.to_string())
                                    .unwrap_or_default(),
                                true,
                            ),
                        };
                    found.push(Member {
                        who,
                        is_group,
                        role: m
                            .get_role()
                            .and_then(|t| t.to_string().map_err(Into::into))
                            .map_err(|e: capnp::Error| problem(&e))?,
                    });
                }
                let next = r
                    .get_next()
                    .map_err(|e| problem(&e))?
                    .get_opaque()
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    return Ok(found);
                }
                cursor = next.to_vec();
            }
        })
    }
}
