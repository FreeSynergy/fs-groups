//! The commands on memberships (`Memberships`).
//!
//! Pattern: Command (one function per command) over a Proxy — as `groups_ops.rs`. A list read with no group named
//! reads every group; the contract asks per group.

use fs_admin_frontend::{Outcome, Record, Values};
use fs_interfaces::groups_capnp::{member, membership};
use fs_interfaces::rpc::write_uuid;
use uuid::Uuid;

use super::groups_ops::all_groups;
use super::input;
use super::read;
use crate::remote::{Clients, GroupsRemote, RunError};

/// The groups a read goes over: the named one, or all.
pub(super) async fn groups_of_query(
    c: &Clients,
    group: Option<Uuid>,
) -> Result<Vec<Uuid>, RunError> {
    Ok(match group {
        Some(g) => vec![g],
        None => all_groups(c)
            .await?
            .iter()
            .filter_map(|g| Uuid::parse_str(&g.id).ok())
            .collect(),
    })
}

pub(super) fn set_member(mut out: member::Builder<'_>, who: &str) {
    match Uuid::parse_str(who) {
        Ok(group) => write_uuid(out.init_group(), Some(group)),
        Err(_) => out.set_person(who),
    }
}

pub(super) fn outcome_id(id: String) -> Outcome {
    Outcome {
        id: Some(id),
        values: Values::new(),
    }
}

pub(super) fn membership_outcome(m: membership::Reader<'_>) -> Result<Outcome, RunError> {
    let mut values = Values::new();
    values.insert("membership".into(), vec![read::membership(m)?.id]);
    Ok(Outcome { id: None, values })
}

/// `Membership.find`.
pub fn find_memberships(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let group = input::id_opt(v, "group")?;
    remote.run(move |c| async move {
        let mut found = Vec::new();
        for g in groups_of_query(&c, group).await? {
            let mut cursor = Vec::<u8>::new();
            loop {
                let mut r = c.memberships.direct_request();
                write_uuid(r.get().init_group(), Some(g));
                r.get().init_cursor().set_opaque(&cursor);
                let answer = r.send().promise.await?;
                let a = answer.get()?;
                for m in a.get_items()? {
                    found.push(read::membership(m)?);
                }
                let next = a.get_next()?.get_opaque()?;
                if next.is_empty() {
                    break;
                }
                cursor = next.to_vec();
            }
        }
        Ok(found)
    })
}

/// `Membership.add`.
pub fn add(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let who = input::need(v, "member")?.to_owned();
    let role = input::one(v, "role").unwrap_or_default().to_owned();
    remote.run(move |c| async move {
        let mut r = c.memberships.add_request();
        write_uuid(r.get().init_group(), Some(group));
        set_member(r.get().init_member(), &who);
        r.get().set_role(role.as_str());
        let answer = r.send().promise.await?;
        Ok(outcome_id(
            read::membership(answer.get()?.get_membership()?)?.id,
        ))
    })
}

/// `Membership.remove`.
pub fn remove(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        let mut r = c.memberships.remove_request();
        write_uuid(r.get().init_membership(), Some(id));
        r.send().promise.await?;
        Ok(Outcome::default())
    })
}

/// `Membership.effective`: persons and, parallel, the path of group ids (space-separated).
pub fn effective(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    remote.run(move |c| async move {
        let (mut persons, mut paths, mut cursor) = (Vec::new(), Vec::new(), Vec::<u8>::new());
        loop {
            let mut r = c.memberships.effective_request();
            write_uuid(r.get().init_group(), Some(group));
            r.get().init_cursor().set_opaque(&cursor);
            let answer = r.send().promise.await?;
            let a = answer.get()?;
            for e in a.get_items()? {
                persons.push(e.get_person()?.to_str()?.to_owned());
                paths.push(read::id_list(e.get_via())?.join(" "));
            }
            let next = a.get_next()?.get_opaque()?;
            if next.is_empty() {
                break;
            }
            cursor = next.to_vec();
        }
        let mut values = Values::new();
        values.insert("persons".into(), persons);
        values.insert("paths".into(), paths);
        Ok(Outcome { id: None, values })
    })
}

/// `Membership.groupsOf`.
pub fn groups_of(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let person = input::need(v, "person")?.to_owned();
    remote.run(move |c| async move {
        let mut r = c.memberships.groups_of_request();
        r.get().set_person(person.as_str());
        let answer = r.send().promise.await?;
        let mut values = Values::new();
        values.insert("ids".into(), read::id_list(answer.get()?.get_ids())?);
        Ok(Outcome { id: None, values })
    })
}

/// `Membership.isMember`.
pub fn is_member(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let person = input::need(v, "person")?.to_owned();
    remote.run(move |c| async move {
        let mut r = c.memberships.is_member_request();
        write_uuid(r.get().init_group(), Some(group));
        r.get().set_person(person.as_str());
        let answer = r.send().promise.await?;
        let mut values = Values::new();
        values.insert(
            "member".into(),
            vec![input::yes_no(answer.get()?.get_member()).into()],
        );
        Ok(Outcome { id: None, values })
    })
}
