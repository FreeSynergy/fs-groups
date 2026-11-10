//! One field or argument as a constant: the row of the tables in `fields.rs` and `commands.rs`.
//!
//! Pattern: Value object with a const builder — a table of rows is data, not code; the builder keeps each row
//! on one line and the rows checkable by the compiler. [`Arg::to_field`] is the only place a row becomes a
//! `fs-types` `FieldDescription`.

use fs_types::{BasicType, FieldDescription};

/// A field of a data type, or an input or result of a command.
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    /// Machine name, lower camel case.
    pub name: &'static str,
    /// The basic type.
    pub basic: BasicType,
    /// A value is required.
    pub required: bool,
    /// A list of values.
    pub multiple: bool,
    /// For `Reference`: the data type referred to; empty = any.
    pub target: &'static str,
    /// For `Choice`: the allowed keys.
    pub choices: &'static [&'static str],
}

/// The yes/no answer: there is no boolean basic type, so it is a choice of two keys.
pub const YES_NO: &[&str] = &["yes", "no"];
/// The rights of a delegation (`Right` in `groups.capnp`), as translation keys (kebab case).
pub const RIGHTS: &[&str] = &["read", "add-members", "remove-members", "manage"];
/// `RightFlow`.
pub const FLOWS: &[&str] = &["none", "down", "up"];
/// `InvitationState`.
pub const INVITATION_STATES: &[&str] = &["pending", "accepted", "declined", "revoked", "expired"];
/// `RequestState`.
pub const REQUEST_STATES: &[&str] = &["pending", "approved", "declined"];
/// `DeliveryState`.
pub const DELIVERY_STATES: &[&str] = &["active", "revoked"];
/// `SourceState`.
pub const SOURCE_STATES: &[&str] = &["active", "withdrawn"];
/// What a group can be told about joining (not in the core yet, see `commands.rs`).
pub const JOIN_POLICIES: &[&str] = &["invitation", "open", "request"];

impl Arg {
    /// A row with the given name and basic type: optional, single, no target, no choices.
    pub const fn new(name: &'static str, basic: BasicType) -> Self {
        Self {
            name,
            basic,
            required: false,
            multiple: false,
            target: "",
            choices: &[],
        }
    }

    /// A reference to a data type.
    pub const fn reference(name: &'static str, target: &'static str) -> Self {
        let mut a = Self::new(name, BasicType::Reference);
        a.target = target;
        a
    }

    /// A choice of the given keys.
    pub const fn choice(name: &'static str, keys: &'static [&'static str]) -> Self {
        let mut a = Self::new(name, BasicType::Choice);
        a.choices = keys;
        a
    }

    /// A value is required.
    #[must_use]
    pub const fn req(mut self) -> Self {
        self.required = true;
        self
    }

    /// A list of values.
    #[must_use]
    pub const fn many(mut self) -> Self {
        self.multiple = true;
        self
    }

    /// The description of this row; its label key is `<scope>.<name>` in kebab case.
    pub fn to_field(&self, scope: &str) -> FieldDescription {
        let mut d = FieldDescription::new(self.name, self.basic)
            .label(&format!("{scope}.{}", kebab(self.name)));
        if self.required {
            d = d.required();
        }
        if self.multiple {
            d = d.multiple();
        }
        if !self.target.is_empty() {
            d = d.target(self.target);
        }
        if !self.choices.is_empty() {
            d = d.choices(self.choices);
        }
        d
    }
}

/// `rightFlow` -> `right-flow`, `Group` -> `group`: the translation key part of a name.
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if !out.is_empty() {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
