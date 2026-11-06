# fs-groups – Design

`fs-groups` is the groups program of FreeSynergy. This repo holds the **program core** (`crates/fs-groups-core`, no GUI).
A group is an **AS2 `Group`** in an envelope stored through `fs-store` (ADR 0014, 0020, 0050). It runs alone, without any
directory (ADR 0024); a directory is only a mirror (ADR 0047). Contract: `groups.capnp` in `fs-interfaces` v0.3.0 (draft).

## State

Version 0.1.0.

| Area | State |
|---|---|
| Group items: create, rename, set kind, get, list (name order, paged by cursor), delete | done |
| Group = AS2 `Group` (envelope type `Group`, `context=groups`, IRI `https://<instance>/groups/<id>`, `name` derived, ADR 0032); kind is free text | done |
| Nesting: several parents, `add_parent` / `remove_parent`, `parents`, `children`, `ancestors`, `descendants` | done |
| A cycle is refused (self-parent, long chains, diamonds closed upward) with an error that names the path | done |
| Memberships as AS2 `Relationship` in `org:Membership` form, role id opaque (definition stays in fs-people) | done |
| Effective membership through nested groups with the `via` path; `groups_of`; `is_member`; ends on any data (tested with a loop planted from outside) | done |
| Delete removes memberships, invitations, delegations (also the ones it holds) and links; children stay | done |
| Invitations: State pattern, one transition function, accept creates the membership, expiry checked against a passed-in time | done |
| Delegation: `grant`, `revoke`, `list`, `may_do` (also through groups, nested included; `manage` implies all) | done; **only answers the question, nothing else enforces it yet** |
| Mirroring: `Mirror::sync_to(&dyn GroupDirectory)` (Template method), idempotent, heals manual changes, removes only ids it mirrored (ledger in the store) | done, tested against an in-memory fake directory |
| `group.subscribe`, capnp-rpc server | **not built**; `capabilities()` says "not built yet; it is possible" |
| Tests | 50 (`tests/*.rs` by feature, real in-memory `TursoStore`) |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `traits/groups.rs` etc. | Repository (interface) + Template method + Null Object | callers use `dyn Groups`; defaults answer `Unsupported`; `ancestors`/`descendants` are provided in terms of `parents`/`children` |
| `traits/directory.rs` | Strategy + Adapter (interface) | the provider interface for mirrors |
| `model/*` | Value objects | plain data, one concept per file |
| `invitation_state.rs` | State | one transition function for the whole life cycle |
| `graph.rs` | Visitor / Iterator | pure breadth-first walks, each group once |
| `repo/handle.rs`, `repo/mod.rs` | Repository + Facade | `GroupRepo` hides store, envelopes and rules |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` as the two narrow traits `ObjectRepository` + `ObjectQuery` |
| `repo/envelope.rs`, `repo/codec.rs` | Adapter | typed values <-> AS2 envelope, content and index fields |
| `repo/nesting.rs` | Specification | the no-cycle rule on the in-memory graph |
| `repo/provide.rs` | Adapter | trait methods call the inherent ones |
| `mirror/sync.rs` | Template method | fixed flow: read, plan, apply, remove, remember |
| `mirror/plan.rs` | Specification (pure) | create / update / keep / remove decided without I/O |
| `repo/mirror.rs` | Repository | wanted state + the ledger of mirrored ids |

## Decisions (the owner may change them)

* **Nesting has one home: `parents`.** The contract allows `Member::group` in `Memberships.add` too, which would be a
  second way to say the same thing (and a second place a cycle could hide). `add` refuses a group member and points to
  `add_parent`. `Member::Group` stays for delegation holders.
* **Group content** is the AS2 `Group` plus the members `fs:kind` and `fs:parents` (namespace `https://freesynergy.net/ns#`).
  Index fields `kind` and `parent` are derived from it. Memberships, invitations and delegations are read back from their
  index fields (`group`, `member`, `role`, `state`, ...); the AS2 content is the standard-shaped view.
* **Effective membership** lists each person once, with the shortest path (`[asked group]` when direct). All paths are not listed.
* **Invitation** has the envelope type `Invite` (AS2 activity), expiry in the envelope `endTime`. `expires = None` never runs out. The
  time is always passed in. `expire_due(now)` is an addition to the contract (a sweep). Answering after the time: stored as `expired`,
  error `Expired`. `decline` and `revoke` ignore the expiry.
* **Delegation**: a right on a parent does not reach children. A member of a holder group holds the right (nested included).
  `Delegation` has the store type `Delegation` (AS2 has none).
* **Mirror**: the ledger (which ids were put into which directory, keyed by the directory's provider name) is a store object of type
  `MirrorLedger`. Roles are not mirrored (a person once per group). Groups are applied children first, so a nested group exists.
* The whole graph is loaded for each nesting / effective question (one query). Fine for the sizes expected; an index of the closure
  is the next step if it is not.
* Empty `kind` and `role` are allowed; the store refuses empty index values, so they are just not indexed.

## Open points

* **Enforcement of rights** (`may_do` is not asked by `add`/`remove`/`delete`): needs the acting person on each call; comes with the capnp server.
* **`group.subscribe`** and the **capnp-rpc server** are not written. The traits return typed values (`Group`, ...), not `StoredObject`.
* The store's invitation-reply activities (`put_and_log` with `Accept` / `Reject`) are not used yet.

## Crates

`fs-store` (git tag v0.2.0), `fs-interfaces` (git tag v0.3.0), `serde_json`, `chrono`, `thiserror`, `async-trait`, `tokio` (sync only; dev: full).
All MIT / Apache-2.0. No Kanidm dependency of any kind.
