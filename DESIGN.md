# fs-groups – Design

`fs-groups` is the groups program of FreeSynergy. This repo holds the **program core** (`crates/fs-groups-core`, no GUI).
A group is an **AS2 `Group`** in an envelope stored through `fs-store` (ADR 0014, 0020, 0050). It runs alone, without any
directory (ADR 0024); a directory is only a mirror (ADR 0047). Contract: `groups.capnp` in `fs-interfaces` v0.7.0 (draft).

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
| Delegation: `grant`, `revoke`, `list`, `may_do` (a right passes on as each group's right flow says; `manage` implies all) | done; **only answers the question, nothing else enforces it yet** |
| Mirroring: `Mirror::sync_to(&dyn GroupDirectory)` (Template method), idempotent, heals manual changes, removes only ids it mirrored (ledger in the store) | done, tested against an in-memory fake directory |
| Right flow (ADR 0055 point 6): `Group.right_flow` (`None` default, `Down`, `Up`), `set_right_flow`, `fs:rightFlow` in the AS2 content (absent = `None`); **meaning provisional** | done |
| Mirror report lists `divergences`: nested edges Kanidm holds against the child's setting; reported, not adapted | done |
| `group.subscribe`, capnp-rpc server | **not built**; `capabilities()` says "not built yet; it is possible" |
| Tests | 61 (`tests/*.rs` by feature, real in-memory `TursoStore`) |

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
| `right_flow.rs` | Strategy (pure) | the one place that says what up and down mean (provisional) and what a directory can hold |
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
* **Right flow (PROVISIONAL; all of it is in `right_flow.rs`, change it there only).** Each group sets whether its rights are
  passed on: `None` (default), `Down` or `Up` (ADR 0055 point 6, replaces "a right on a parent does not reach its children").
  `None`: a right on G counts on G only; a holder group's right reaches its direct members only. `Down`: the right also counts
  on G's children, onward while each group on the way also passes down; a holder group's right also reaches the members of
  those children. `Up`: the same toward parents. The group the right passes FROM decides; the walk keeps the direction of the
  group holding the right. Each group is visited once (ends on cyclic data). Membership (`effective`, `groups_of`, `is_member`)
  is about belonging and does not use the setting. `Manage` still implies the other rights.
* **Delegation**: see right flow for who is reached. `Delegation` has the store type `Delegation` (AS2 has none).
* **Mirror and right flow**: in Kanidm a member of a child is always `memberof` the parent, per group not switchable. The mirror
  does not adapt: it applies the nested edges as before and `SyncReport::divergences` names each child group (and its parent)
  whose setting is not `Up` (provisional reading of "the child's rights would not pass that way", `directory_expresses`).
  Whether to stop mirroring such edges is the owner's later decision; not built.
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
