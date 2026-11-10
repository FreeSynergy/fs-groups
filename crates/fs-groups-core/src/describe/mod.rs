//! The type descriptions of the program (ADR 0066): what a generated frontend is told about the data types and
//! their commands.
//!
//! Pattern: Registry (tables in `arg.rs`, `fields.rs`, `commands.rs`) + Adapter (`types.rs` turns rows into
//! `fs-types` descriptions). The table of commands is the single source (see its head).

mod arg;
mod commands;
mod fields;
mod types;

pub use arg::Arg;
pub use commands::{
    COMMANDS, Command, NOT_COMMANDS, commands_of, find_command, unsupported_commands,
};
pub use fields::{
    DELEGATION, DELIVERY, GROUP, INVITATION, MEMBERSHIP, PARENT_REQUEST, PROGRAM, SOURCE, TYPES,
    fields_of,
};
pub use types::{describe_named, registry};
