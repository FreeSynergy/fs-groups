//! Following between independent groups (ADR 0056, "Einseitig folgen"): which pattern, and why.
//!
//! Pattern: Specification (this file, pure predicates) + State (the life of a delivery and of a source)
//! + Repository (the sources a group takes in, the deliveries it makes).
//!
//! Following is **not** nesting: there are no parents and no tree, so `graph.rs` and `nesting.rs`
//! are not touched.
//!
//! * *Repository, turned round:* the giver **delivers**, the receiver **keeps a list of sources**. A
//!   group's sources are what it has been offered, like the remotes of a version-control repository.
//! * *State:* a delivery is `Active` until the giver revokes it; a source is `Active` until the giver
//!   withdraws it (the receiver may also remove it at any time). The offer reaches the receiver as an
//!   AS2 `Invite` (proposal, the owner may change the form); it is entered without any confirmation.
//! * *Specification:* "never more than the giver has", "pass on only with the source's permission" and
//!   "the offer comes from where the transport says it comes from" are pure yes/no rules over values,
//!   in one place, tested without a store.
//!
//! No other pattern of `coding.md` section 2 fits better; nothing was invented.
