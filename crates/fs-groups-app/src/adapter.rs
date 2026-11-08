//! `GroupsAdapter`: [`GroupsApi`] as the workbench's `PeopleSource`.
//!
//! Pattern: Adapter — the list-and-page workbench of `fs-components` (made for people, neutral in
//! what it shows) reads groups as rows (name, kind), a sheet (kind, what it is part of, its direct
//! members) and a form (name, kind). Everything else the groups program does (nesting requests,
//! invitations, rights, mirrors) is not in this window yet.

use std::sync::Arc;

use fs_components_workbenches::SourceError;
use fs_components_workbenches::people::{
    DraftField, DraftGroup, PeopleCapabilities, PeopleSource, PersonDraft, PersonField, PersonRow,
    PersonSection, PersonSheet,
};

use crate::remote::{ApiError, Group, GroupsApi};
use crate::words::field;

const NAME: &str = "name";
const KIND: &str = "kind";

/// The groups of a program as the workbench's source.
pub struct GroupsAdapter {
    api: Arc<dyn GroupsApi>,
}

impl GroupsAdapter {
    /// An adapter over `api`.
    pub fn new(api: Arc<dyn GroupsApi>) -> Self {
        Self { api }
    }
}

fn reason(error: ApiError) -> SourceError {
    match error {
        ApiError::NotFound(m) => SourceError::not_found(m),
        ApiError::Unsupported(m) => SourceError::unsupported(m),
        ApiError::Failed(m) => SourceError::failed(m),
    }
}

fn matches(group: &Group, term: &str) -> bool {
    let term = term.trim().to_lowercase();
    term.is_empty()
        || group.name.to_lowercase().contains(&term)
        || group.kind.to_lowercase().contains(&term)
}

fn form(name: &str, kind: &str, revision: Option<String>) -> PersonDraft {
    PersonDraft::new(
        revision,
        vec![
            DraftGroup::new(
                NAME,
                field::NAME,
                false,
                vec![DraftField::new(NAME, field::NAME, name)],
            ),
            DraftGroup::new(
                KIND,
                field::KIND,
                false,
                vec![DraftField::new(KIND, field::KIND, kind)],
            ),
        ],
    )
}

fn value_of<'a>(draft: &'a PersonDraft, key: &str) -> &'a str {
    draft
        .fields()
        .find(|f| f.key == key)
        .map_or("", |f| f.value.trim())
}

impl PeopleSource for GroupsAdapter {
    fn capabilities(&self) -> PeopleCapabilities {
        let mut can = PeopleCapabilities::read_only();
        can.edit = true;
        can.create = true;
        can.delete = true;
        can
    }

    fn rows(&self, term: &str, offset: usize, limit: usize) -> Result<Vec<PersonRow>, SourceError> {
        Ok(self
            .api
            .list()
            .map_err(reason)?
            .into_iter()
            .filter(|g| matches(g, term))
            .skip(offset)
            .take(limit)
            .map(|g| PersonRow::new(g.id, vec![g.name, g.kind]))
            .collect())
    }

    fn read(&self, key: &str) -> Result<PersonSheet, SourceError> {
        let group = self
            .api
            .get(key)
            .map_err(reason)?
            .ok_or_else(|| SourceError::not_found("This group no longer exists."))?;
        let mut sections = vec![PersonSection::new(
            field::DETAILS,
            vec![PersonField::new(field::KIND, group.kind.clone())],
        )];
        let mut parents = Vec::new();
        for id in &group.parents {
            // A parent that cannot be read is still named by its id: the list stays whole.
            let name = self
                .api
                .get(id)
                .ok()
                .flatten()
                .map_or_else(|| id.clone(), |p| p.name);
            parents.push(PersonField::new(field::GROUP_OF, name));
        }
        if !parents.is_empty() {
            sections.push(PersonSection::new(field::PARENTS, parents));
        }
        let members = self.api.members(key).map_err(reason)?;
        let fields: Vec<PersonField> = members
            .into_iter()
            .map(|m| {
                let who = if m.is_group {
                    self.api
                        .get(&m.who)
                        .ok()
                        .flatten()
                        .map_or(m.who.clone(), |g| g.name)
                } else {
                    m.who
                };
                let role = if m.role.is_empty() {
                    field::PLAIN_MEMBER.to_string()
                } else {
                    m.role
                };
                PersonField::new(role, who)
            })
            .collect();
        if !fields.is_empty() {
            sections.push(PersonSection::new(field::MEMBERS, fields));
        }
        Ok(PersonSheet::new(group.name, sections))
    }

    fn draft(&self, key: &str) -> Result<PersonDraft, SourceError> {
        let group = self
            .api
            .get(key)
            .map_err(reason)?
            .ok_or_else(|| SourceError::not_found("This group no longer exists."))?;
        // A group has no revision on the wire: a change by someone else is not detected.
        Ok(form(&group.name, &group.kind, None))
    }

    fn blank(&self) -> Result<PersonDraft, SourceError> {
        Ok(form("", "", None))
    }

    fn save(&self, key: &str, draft: &PersonDraft) -> Result<(), SourceError> {
        let (name, kind) = (value_of(draft, NAME), value_of(draft, KIND));
        if name.is_empty() {
            return Err(SourceError::failed(field::NO_NAME));
        }
        let was = self
            .api
            .get(key)
            .map_err(reason)?
            .ok_or_else(|| SourceError::not_found("This group no longer exists."))?;
        if name != was.name {
            self.api.rename(key, name).map_err(reason)?;
        }
        if kind != was.kind {
            self.api.set_kind(key, kind).map_err(reason)?;
        }
        Ok(())
    }

    fn create(&self, draft: &PersonDraft) -> Result<String, SourceError> {
        let (name, kind) = (value_of(draft, NAME), value_of(draft, KIND));
        if name.is_empty() {
            return Err(SourceError::failed(field::NO_NAME));
        }
        self.api.create(name, kind).map(|g| g.id).map_err(reason)
    }

    fn delete(&self, key: &str) -> Result<Option<String>, SourceError> {
        self.api.delete(key).map(|()| None).map_err(reason)
    }
}
