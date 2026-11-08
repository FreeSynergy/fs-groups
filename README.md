# fs-groups

FreeSynergy program for groups: nested groups, who belongs to them, invitations and fine-grained rights. Runs alone (ADR 0024); a directory such as Kanidm is only a mirror behind the `GroupDirectory` interface (ADR 0047).

* `crates/fs-groups-core`: the program core (no GUI). Start with `GroupRepo` and the traits `Groups`, `Memberships`, `Invitations`, `Delegations`, `GroupDirectory`. See `DESIGN.md`.
* `crates/fs-groups-server`: the contract over a Unix socket (`fs-groups-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]`). It does not authenticate callers: `--actor` is a name, not a proof (`DESIGN.md`).
* The contract is `groups.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (see `DESIGN.md` for the tag the core builds on).
* The Kanidm adapter lives in [fs-identity](https://github.com/FreeSynergy/fs-identity) (name provisional).

Licence: MIT OR Apache-2.0.
