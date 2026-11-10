//! The commands on invitations (`Invitations`).
//!
//! Pattern: Command (one function per command) over a Proxy — as `groups_ops.rs`.

use fs_admin_frontend::{Outcome, Record, Values};
use fs_interfaces::rpc::write_uuid;

use super::input;
use super::members_ops::{groups_of_query, membership_outcome, outcome_id};
use super::read;
use crate::remote::{GroupsRemote, RunError};

/// `Invitation.find`.
pub fn find_invitations(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let group = input::id_opt(v, "group")?;
    remote.run(move |c| async move {
        let mut found = Vec::new();
        for g in groups_of_query(&c, group).await? {
            let mut r = c.invitations.list_request();
            write_uuid(r.get().init_group(), Some(g));
            let answer = r.send().promise.await?;
            for i in answer.get()?.get_items()? {
                found.push(read::invitation(i)?);
            }
        }
        Ok(found)
    })
}

/// `Invitation.invite`.
pub fn invite(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let invitee = input::need(v, "invitee")?.to_owned();
    let role = input::one(v, "role").unwrap_or_default().to_owned();
    let expires = input::millis_opt(v, "expires")?;
    remote.run(move |c| async move {
        let mut r = c.invitations.invite_request();
        write_uuid(r.get().init_group(), Some(group));
        r.get().set_invitee(invitee.as_str());
        r.get().set_role(role.as_str());
        if let Some(ms) = expires {
            r.get().init_expires().set_unix_millis(ms);
        }
        let answer = r.send().promise.await?;
        Ok(outcome_id(
            read::invitation(answer.get()?.get_invitation()?)?.id,
        ))
    })
}

/// `accept` and `acceptProved`.
pub fn accept(remote: &GroupsRemote, v: &Values, proved: bool) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    let person = input::need(v, "person")?.to_owned();
    let address = if proved {
        input::need(v, "provedAddress")?.to_owned()
    } else {
        String::new()
    };
    remote.run(move |c| async move {
        if proved {
            let mut r = c.invitations.accept_proved_request();
            write_uuid(r.get().init_id(), Some(id));
            r.get().set_person(person.as_str());
            r.get().set_proved_address(address.as_str());
            let answer = r.send().promise.await?;
            membership_outcome(answer.get()?.get_membership()?)
        } else {
            let mut r = c.invitations.accept_request();
            write_uuid(r.get().init_id(), Some(id));
            r.get().set_person(person.as_str());
            let answer = r.send().promise.await?;
            membership_outcome(answer.get()?.get_membership()?)
        }
    })
}

/// `decline` and `revoke` of an invitation.
pub fn end_invitation(
    remote: &GroupsRemote,
    v: &Values,
    decline: bool,
) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        if decline {
            let mut r = c.invitations.decline_request();
            write_uuid(r.get().init_id(), Some(id));
            r.send().promise.await?;
        } else {
            let mut r = c.invitations.revoke_request();
            write_uuid(r.get().init_id(), Some(id));
            r.send().promise.await?;
        }
        Ok(Outcome::default())
    })
}
