//! The bridge from the people role book to `fs_groups_core::AiMarking`.
//!
//! Pattern: Adapter — see [`marking`]; this file only lists the module and re-exports.

mod marking;

pub use marking::PeopleMarking;
