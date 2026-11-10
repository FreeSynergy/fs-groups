//! The type descriptions of the program: `DescribesType` for every data type it owns (ADR 0066).
//!
//! Pattern: Registry entry (trait first) — each model type implements `DescribesType` from `fs-types`, so
//! `TypeRegistry::register::<Group>()` puts it in the catalog `Groups.types` serves. Fields and commands both come
//! from the tables (`fields.rs`, `commands.rs`); this file only turns rows into descriptions.
//!
//! The description holds **all** commands of the table, also those the program cannot run yet. fs-types has no
//! `supported` field on a command yet, so the flag does not travel on the wire: a backend refuses the commands of
//! [`unsupported_commands`](super::commands::unsupported_commands) itself (see `DESIGN.md`, "Unsupported").

use fs_types::{
    CommandDescription, DescribesType, DescriptionProblem, TypeDescription, TypeRegistry,
};

use super::arg::kebab;
use super::commands::{Command, commands_of};
use super::fields::{
    DELEGATION, DELIVERY, GROUP, INVITATION, MEMBERSHIP, PARENT_REQUEST, PROGRAM, SOURCE, TYPES,
    fields_of,
};
use crate::model::{Delegation, Delivery, Group, Invitation, Membership, ParentRequest, Source};

fn command_description(c: &Command) -> CommandDescription {
    let scope = format!("{}.{}", kebab(c.data_type), kebab(c.name));
    let d = CommandDescription::new(c.name, c.kind, &scope);
    let d = c.input.iter().fold(d, |d, a| d.input(a.to_field(&scope)));
    let d = c.result.iter().fold(d, |d, a| d.result(a.to_field(&scope)));
    // SWITCH: set .supported(c.supported) when fs-types has it
    d
}

/// The description of the data type `name`, built from the tables.
///
/// # Panics
/// On a name that is not in [`TYPES`]; the trait implementations below only pass their own.
pub fn describe_named(name: &str, standard: &str) -> TypeDescription {
    assert!(TYPES.contains(&name), "unknown data type {name}");
    let label = kebab(name);
    let mut t = TypeDescription::new(name, PROGRAM, standard, &label);
    for f in fields_of(name) {
        t = t.field(f.to_field(&label));
    }
    for c in commands_of(name) {
        t = t.command(command_description(c));
    }
    t
}

macro_rules! describes {
    ($ty:ty, $name:expr, $standard:expr) => {
        impl DescribesType for $ty {
            fn describe() -> TypeDescription {
                describe_named($name, $standard)
            }
        }
    };
}

describes!(Group, GROUP, "AS2 Group");
describes!(Membership, MEMBERSHIP, "org:Membership (W3C Organization Ontology)");
describes!(Invitation, INVITATION, "AS2 Invite");
describes!(Delegation, DELEGATION, "own specification (gap, groups.md)");
describes!(ParentRequest, PARENT_REQUEST, "own specification (gap, groups.md)");
describes!(Delivery, DELIVERY, "AS2 Invite (offer), own specification");
describes!(Source, SOURCE, "AS2 Invite (offer), own specification");

/// The registry of all data types of the program: what `Groups.types` answers.
///
/// # Errors
/// [`DescriptionProblem`] when a description breaks the rules (a defect of the tables; a test covers it).
pub fn registry() -> Result<TypeRegistry, DescriptionProblem> {
    let mut r = TypeRegistry::new();
    r.register::<Group>()?
        .register::<Membership>()?
        .register::<Invitation>()?
        .register::<Delegation>()?
        .register::<ParentRequest>()?
        .register::<Delivery>()?
        .register::<Source>()?;
    Ok(r)
}
