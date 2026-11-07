//! Who belongs: a person or a group, memberships and effective members.
//!
//! Pattern: Value objects — `Member` is a sum type (the `union` of the contract).

use fs_objects::ObjectId;

/// A person (IRI of a person in fs-people, or an actor) or a group.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Member {
    /// A person, by IRI. Opaque to this crate.
    Person(String),
    /// A group, by id.
    Group(ObjectId),
}

impl Member {
    /// The kind written to the index: `person` or `group`.
    pub fn kind(&self) -> &'static str {
        match self {
            Member::Person(_) => "person",
            Member::Group(_) => "group",
        }
    }

    /// The value written to the index: the IRI, or the group id.
    pub fn key(&self) -> String {
        match self {
            Member::Person(p) => p.clone(),
            Member::Group(g) => g.to_string(),
        }
    }

    /// Inverse of [`Member::kind`] and [`Member::key`].
    pub fn from_parts(kind: &str, key: &str) -> Option<Member> {
        match kind {
            "person" => Some(Member::Person(key.to_string())),
            "group" => key.parse().ok().map(Member::Group),
            _ => None,
        }
    }
}

/// `org:Membership`: a member holds a role in a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    /// Id of the membership.
    pub id: ObjectId,
    /// The group.
    pub group: ObjectId,
    /// The member (a person; see `DESIGN.md` for why not a group).
    pub member: Member,
    /// Role id of `people.capnp`, opaque here; empty = plain member.
    pub role: String,
    /// `false` when the role is a configured admin role and the person is an agent or not known to
    /// be a human (ADR 0056): the entry stays on record but counts for nothing. Always `true` for
    /// other roles.
    pub in_force: bool,
}

/// A person who belongs to a group, and through whom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveMember {
    /// The person.
    pub person: String,
    /// Groups from the asked group down to the one with the direct membership; just the asked
    /// group when the person is a direct member.
    pub via: Vec<ObjectId>,
}
