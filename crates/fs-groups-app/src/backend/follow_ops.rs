//! The commands of following between independent groups (`Follows`, provisional in the contract).
//!
//! Pattern: Command (one function per command) over a Proxy — as `groups_ops.rs`.

use fs_admin_frontend::{Outcome, Record, Values};
use fs_interfaces::groups_capnp::Right;
use fs_interfaces::rpc::write_uuid;
use uuid::Uuid;

use super::groups_ops::all_groups;
use super::input;
use super::read;
use crate::remote::{GroupsRemote, RunError};

fn write_rights<'a>(
    init: impl FnOnce(u32) -> capnp::enum_list::Builder<'a, Right>,
    rights: &[Right],
) {
    let mut out = init(u32::try_from(rights.len()).unwrap_or(u32::MAX));
    for (i, r) in (0u32..).zip(rights) {
        out.set(i, *r);
    }
}

fn outcome_id(id: String) -> Outcome {
    Outcome {
        id: Some(id),
        values: Values::new(),
    }
}

async fn groups(c: &crate::remote::Clients, group: Option<Uuid>) -> Result<Vec<Uuid>, RunError> {
    Ok(match group {
        Some(g) => vec![g],
        None => all_groups(c)
            .await?
            .iter()
            .filter_map(|g| Uuid::parse_str(&g.id).ok())
            .collect(),
    })
}

/// `Delivery.find`.
pub fn find_deliveries(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let group = input::id_opt(v, "group")?;
    remote.run(move |c| async move {
        let mut found = Vec::new();
        for g in groups(&c, group).await? {
            let mut r = c.follows.deliveries_request();
            write_uuid(r.get().init_group(), Some(g));
            let answer = r.send().promise.await?;
            for d in answer.get()?.get_items()? {
                found.push(read::delivery(d)?);
            }
        }
        Ok(found)
    })
}

/// `Source.find`.
pub fn find_sources(remote: &GroupsRemote, v: &Values) -> Result<Vec<Record>, RunError> {
    let group = input::id_opt(v, "group")?;
    remote.run(move |c| async move {
        let mut found = Vec::new();
        for g in groups(&c, group).await? {
            let mut r = c.follows.sources_request();
            write_uuid(r.get().init_group(), Some(g));
            let answer = r.send().promise.await?;
            for s in answer.get()?.get_items()? {
                found.push(read::source(s)?);
            }
        }
        Ok(found)
    })
}

/// `Delivery.deliver`.
pub fn deliver(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let group = input::id(v, "group")?;
    let recipient = input::need(v, "recipient")?.to_owned();
    let rights = input::rights(v, "rights")?;
    let pass_on = input::is_yes(v, "passOn")?;
    remote.run(move |c| async move {
        let mut r = c.follows.deliver_request();
        write_uuid(r.get().init_group(), Some(group));
        r.get().set_recipient(recipient.as_str());
        write_rights(|n| r.get().init_rights(n), &rights);
        r.get().set_pass_on(pass_on);
        let answer = r.send().promise.await?;
        Ok(outcome_id(
            read::delivery(answer.get()?.get_delivery()?)?.id,
        ))
    })
}

/// `Delivery.relay` (`passOn` of the contract).
pub fn relay(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let source = input::id(v, "source")?;
    let recipient = input::need(v, "recipient")?.to_owned();
    let rights = input::rights(v, "rights")?;
    remote.run(move |c| async move {
        let mut r = c.follows.pass_on_request();
        write_uuid(r.get().init_source(), Some(source));
        r.get().set_recipient(recipient.as_str());
        write_rights(|n| r.get().init_rights(n), &rights);
        let answer = r.send().promise.await?;
        Ok(outcome_id(
            read::delivery(answer.get()?.get_delivery()?)?.id,
        ))
    })
}

/// `Delivery.revoke`.
pub fn revoke(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        let mut r = c.follows.revoke_request();
        write_uuid(r.get().init_delivery(), Some(id));
        r.send().promise.await?;
        Ok(Outcome::default())
    })
}

/// `Source.receive`.
pub fn receive(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let sender = input::need(v, "sender")?.to_owned();
    let delivery = input::id(v, "delivery")?;
    let source = input::need(v, "source")?.to_owned();
    let recipient = input::need(v, "recipient")?.to_owned();
    let rights = input::rights(v, "rights")?;
    let pass_on = input::is_yes(v, "passOn")?;
    let origin = input::one(v, "origin").map_or_else(|| source.clone(), str::to_owned);
    remote.run(move |c| async move {
        let mut r = c.follows.receive_request();
        r.get().set_sender(sender.as_str());
        let mut offer = r.get().init_offer();
        write_uuid(offer.reborrow().init_delivery(), Some(delivery));
        offer.set_source(source.as_str());
        offer.set_recipient(recipient.as_str());
        write_rights(|n| offer.reborrow().init_rights(n), &rights);
        offer.set_pass_on(pass_on);
        offer.set_origin(origin.as_str());
        let answer = r.send().promise.await?;
        Ok(outcome_id(read::source(answer.get()?.get_source()?)?.id))
    })
}

/// `Source.withdraw`.
pub fn withdraw(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let sender = input::need(v, "sender")?.to_owned();
    let source = input::need(v, "source")?.to_owned();
    let delivery = input::id(v, "delivery")?;
    remote.run(move |c| async move {
        let mut r = c.follows.withdraw_request();
        r.get().set_sender(sender.as_str());
        r.get().set_source(source.as_str());
        write_uuid(r.get().init_delivery(), Some(delivery));
        let answer = r.send().promise.await?;
        Ok(outcome_id(read::source(answer.get()?.get_source()?)?.id))
    })
}

/// `Source.remove`.
pub fn remove_source(remote: &GroupsRemote, v: &Values) -> Result<Outcome, RunError> {
    let id = input::id(v, "id")?;
    remote.run(move |c| async move {
        let mut r = c.follows.remove_source_request();
        write_uuid(r.get().init_id(), Some(id));
        r.send().promise.await?;
        Ok(Outcome::default())
    })
}
