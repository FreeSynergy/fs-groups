//! The commands on delegations (`Delegations`).
//!
//! Pattern: Command (one function per command) over a Proxy — as `groups_ops.rs`.

use fs_admin_frontend::{Outcome, Record, Values};
use fs_interfaces::rpc::write_uuid;

use super::input;
use super::members_ops::{groups_of_query, outcome_id, set_member};
use super::read;
use crate::remote::{GroupsRemote, RunError};

/// `Delegation.find`.
pub fn find_delegations(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let group = input::id_opt(v, "group")?;
    remote.run(move |c| async move {
        let mut found = Vec::new();
        for g in groups_of_query(&c, group).await? {
            let mut r = c.delegations.list_request();
            write_uuid(r.get().init_group(), Some(g));
            let answer = r.send().promise.await?;
            for d in answer.get()?.get_items()? {
                found.push(read::delegation(d)?);
            }
        }
        Ok(found)
    })
}

/// `Delegation.grant`.
pub fn grant(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let holder = input::need(v, "holder")?.to_owned();
    let right = input::right(input::need(v, "right")?, "right")?;
    remote.run(move |c| async move {
        let mut r = c.delegations.grant_request();
        write_uuid(r.get().init_group(), Some(group));
        set_member(r.get().init_holder(), &holder);
        r.get().set_right(right);
        let answer = r.send().promise.await?;
        Ok(outcome_id(
            read::delegation(answer.get()?.get_delegation()?)?.id,
        ))
    })
}

/// `Delegation.revoke`.
pub fn revoke(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        let mut r = c.delegations.revoke_request();
        write_uuid(r.get().init_id(), Some(id));
        r.send().promise.await?;
        Ok(Outcome::default())
    })
}

/// `Delegation.mayDo`.
pub fn may_do(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let person = input::need(v, "person")?.to_owned();
    let right = input::right(input::need(v, "right")?, "right")?;
    remote.run(move |c| async move {
        let mut r = c.delegations.may_do_request();
        write_uuid(r.get().init_group(), Some(group));
        r.get().set_person(person.as_str());
        r.get().set_right(right);
        let answer = r.send().promise.await?;
        let mut values = Values::new();
        values.insert(
            "allowed".into(),
            vec![input::yes_no(answer.get()?.get_allowed()).into()],
        );
        Ok(Outcome { id: None, values })
    })
}
