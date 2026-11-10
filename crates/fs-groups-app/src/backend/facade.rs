//! `GroupsBackend`: the frontend's `Backend` over the groups server (see the head of `mod.rs`).
//!
//! Pattern: Repository (frontend trait) as Facade over the Command functions.

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_groups_core::describe::{self, find_command};
use fs_types::{Edit, RecordIssue, RecordProblem, TypeProblem};

use super::{delegations_ops, follow_ops, groups_ops, invitations_ops, members_ops};
use crate::remote::{GroupsRemote, RunError};

/// The records and commands of one data type of the program.
pub struct GroupsBackend {
    remote: Arc<GroupsRemote>,
    data_type: &'static str,
}

impl GroupsBackend {
    /// The backend of `data_type` (one of `fs_groups_core::describe::TYPES`).
    pub fn new(remote: Arc<GroupsRemote>, data_type: &'static str) -> Self {
        Self { remote, data_type }
    }

    /// The records `find` selects.
    ///
    /// # Errors
    /// [`RunError`].
    pub fn list_typed(&self, query: &Values) -> Result<Vec<Record>, RunError> {
        let r = &*self.remote;
        match self.data_type {
            describe::GROUP => groups_ops::find(r, query),
            describe::PARENT_REQUEST => groups_ops::parent_requests(r, query),
            describe::MEMBERSHIP => members_ops::find_memberships(r, query),
            describe::INVITATION => invitations_ops::find_invitations(r, query),
            describe::DELEGATION => delegations_ops::find_delegations(r, query),
            describe::DELIVERY => follow_ops::find_deliveries(r, query),
            describe::SOURCE => follow_ops::find_sources(r, query),
            _ => Err(RunError::NotFound),
        }
    }

    /// Carry out the command `name` (`record` is the record a record command works on).
    ///
    /// # Errors
    /// [`RunError::Unsupported`] for a command the table marks as not there yet (nothing is sent),
    /// [`RunError::NotFound`] for a name that is not in the table, else what the call gives.
    pub fn run_typed(
        &self,
        name: &str,
        record: Option<&str>,
        input: &Values,
    ) -> Result<Outcome, RunError> {
        let command = find_command(self.data_type, name).ok_or(RunError::NotFound)?;
        if !command.supported {
            return Err(RunError::Unsupported(format!("{}.{name}", self.data_type)));
        }
        // The id travels next to the input; the command reads it from its reference to the own type.
        let mut with_record = input.clone();
        if let Some(id) = record
            && let Some(own) = command
                .input
                .iter()
                .find(|a| a.target == self.data_type && a.name != "parent" && a.name != "child")
        {
            with_record.insert(own.name.to_owned(), vec![id.to_owned()]);
        }
        let v = &with_record;
        let r = &*self.remote;
        match (self.data_type, name) {
            (_, "find") => {
                let mut values = Values::new();
                values.insert(
                    "items".into(),
                    self.list_typed(v)?.into_iter().map(|rec| rec.id).collect(),
                );
                Ok(Outcome { id: None, values })
            }
            (describe::GROUP, "create") => groups_ops::create(r, v),
            (describe::GROUP, "delete") => groups_ops::delete(r, v),
            (describe::GROUP, "removeParent") => groups_ops::remove_parent(r, v),
            (describe::GROUP, which @ ("parents" | "children" | "ancestors" | "descendants")) => {
                groups_ops::related(r, v, which_static(which))
            }
            (describe::PARENT_REQUEST, "create") => groups_ops::request_parent(r, v),
            (describe::PARENT_REQUEST, "approve") => groups_ops::answer_request(r, v, true),
            (describe::PARENT_REQUEST, "decline") => groups_ops::answer_request(r, v, false),
            (describe::MEMBERSHIP, "add") => members_ops::add(r, v),
            (describe::MEMBERSHIP, "remove") => members_ops::remove(r, v),
            (describe::MEMBERSHIP, "effective") => members_ops::effective(r, v),
            (describe::MEMBERSHIP, "groupsOf") => members_ops::groups_of(r, v),
            (describe::MEMBERSHIP, "isMember") => members_ops::is_member(r, v),
            (describe::INVITATION, "invite") => invitations_ops::invite(r, v),
            (describe::INVITATION, "accept") => invitations_ops::accept(r, v, false),
            (describe::INVITATION, "acceptProved") => invitations_ops::accept(r, v, true),
            (describe::INVITATION, "decline") => invitations_ops::end_invitation(r, v, true),
            (describe::INVITATION, "revoke") => invitations_ops::end_invitation(r, v, false),
            (describe::DELEGATION, "grant") => delegations_ops::grant(r, v),
            (describe::DELEGATION, "revoke") => delegations_ops::revoke(r, v),
            (describe::DELEGATION, "mayDo") => delegations_ops::may_do(r, v),
            (describe::DELIVERY, "deliver") => follow_ops::deliver(r, v),
            (describe::DELIVERY, "relay") => follow_ops::relay(r, v),
            (describe::DELIVERY, "revoke") => follow_ops::revoke(r, v),
            (describe::SOURCE, "receive") => follow_ops::receive(r, v),
            (describe::SOURCE, "withdraw") => follow_ops::withdraw(r, v),
            (describe::SOURCE, "remove") => follow_ops::remove_source(r, v),
            // In the table as supported but not wired here: a defect, said as such (a test walks the table).
            _ => Err(RunError::Unavailable(format!(
                "{}.{name} is in the table but not wired",
                self.data_type
            ))),
        }
    }
}

fn which_static(name: &str) -> &'static str {
    match name {
        "parents" => "parents",
        "children" => "children",
        "ancestors" => "ancestors",
        _ => "descendants",
    }
}

/// The finding for a field the program refused or could not use.
fn issue(field: &str) -> RecordIssue {
    RecordIssue {
        field: field.to_owned(),
        index: 0,
        problem: RecordProblem::Value(TypeProblem::NotInList),
    }
}

/// The frontend has no variant for "the program refused by its rules" (a cycle, forbidden, expired): the closest is
/// `Rejected`, on the first field of the input that was sent. Reported in `DESIGN.md`.
fn to_backend(error: RunError, data_type: &str, name: &str, input: &Values) -> BackendError {
    match error {
        RunError::Unsupported(_) | RunError::NotFound => BackendError::NotFound,
        RunError::Bad { field } => BackendError::Rejected(vec![issue(&field)]),
        RunError::Refused { kind, detail } => {
            let field = find_command(data_type, name)
                .and_then(|c| c.input.iter().find(|a| input.contains_key(a.name)))
                .map_or("id", |a| a.name);
            let _ = (kind, detail);
            BackendError::Rejected(vec![issue(field)])
        }
        RunError::Unavailable(text) => BackendError::Unavailable(text),
    }
}

impl Backend for GroupsBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        self.list_typed(query)
            .map_err(|e| to_backend(e, self.data_type, "find", query))
    }

    /// The table decides, so a command that is not there yet is locked in the frontend without a change to it.
    fn supported(&self, command: &str) -> bool {
        find_command(self.data_type, command).is_some_and(|c| c.supported)
    }

    fn change(&self, record: &str, edit: &Edit) -> Result<Outcome, BackendError> {
        let none = Values::new();
        match self.data_type {
            describe::GROUP => groups_ops::change(&self.remote, record, edit),
            // The other types have no `change` in the table: nothing to apply it to.
            _ => Err(RunError::Unsupported(format!("{}.change", self.data_type))),
        }
        .map_err(|e| to_backend(e, self.data_type, "change", &none))
    }

    fn run(
        &self,
        name: &str,
        record: Option<&str>,
        input: &Values,
    ) -> Result<Outcome, BackendError> {
        self.run_typed(name, record, input)
            .map_err(|e| to_backend(e, self.data_type, name, input))
    }
}
