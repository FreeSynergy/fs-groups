# fs-groups – Design

`fs-groups` is the groups program of FreeSynergy. This repo holds the **program core** (`crates/fs-groups-core`, no GUI).
A group is an **AS2 `Group`** in an envelope stored through `fs-store` (ADR 0014, 0020, 0050). It runs alone, without any
directory (ADR 0024); a directory is only a mirror (ADR 0047). Contract: `groups.capnp` in `fs-interfaces` v0.19.0 (draft).

## State

Version 0.2.0. **Several rules below are provisional: the owner is still answering what up and down mean and who consents to a nesting link.** No tag of this repo until he has.

| Area | State |
|---|---|
| Group items: create, rename, set kind, get, list (name order, paged by cursor), delete | done |
| Group = AS2 `Group` (envelope type `Group`, `context=groups`, IRI `https://<instance>/groups/<id>`, `name` derived, ADR 0032); kind is free text | done |
| Nesting: several parents, `request_parent` / `approve_parent` / `decline_parent` / `parent_requests` (the parent consents, **provisional**), `remove_parent`, `parents`, `children`, `ancestors`, `descendants` | done |
| A cycle is refused (self-parent, long chains, diamonds closed upward) with an error that names the path, when asking and again when approving | done |
| Memberships as AS2 `Relationship` in `org:Membership` form, role id opaque (definition stays in fs-people) | done |
| Effective membership through nested groups with the `via` path; `groups_of`; `is_member`; ends on any data (tested with a loop planted from outside) | done |
| Delete removes memberships, invitations, delegations (also the ones it holds) and links; children stay | done |
| Invitations: State pattern, one transition function, accept creates the membership **for the invitee only**, expiry checked against a passed-in time; `accept_proved` for an e-mail invitee (**provisional**) | done |
| Delegation: `grant` (idempotent), `revoke`, `list`, `may_do` (safe reading of the right flow, in doubt no; `manage` implies all) | done; **only answers the question, nothing else enforces it yet** |
| Mirroring: `Mirror::sync_to(&dyn GroupDirectory)` (Template method), idempotent, heals manual changes, removes only ids it mirrored (ledger in the store) | done, tested against an in-memory fake directory |
| Right flow (ADR 0055 point 6): `Group.right_flow` (`None` default, `Down`, `Up`), `set_right_flow`, `fs:rightFlow` in the AS2 content (absent = `None`); **meaning provisional** | done |
| Mirror report lists `divergences`: nested edges where the directory gives more than `may_do` (with the safe reading: every edge); reported, not adapted | done |
| Mirror hardening: ledger written before the directory (intent) and tidied after; keyed by `directory_id`; unreadable groups never removed; a parent applied only with children the directory holds; one unmapped person does not block the group | done |
| One write path (`Writer` holds the lock), process-wide lock per store handle | done; **single-writer limit, see Decisions** |
| `group.subscribe`, capnp-rpc server | **not built**; `capabilities()` says "not built yet; it is possible" |
| Tests | see the final count in the commit message (`tests/*.rs` by feature, real in-memory `TursoStore`, forced interleavings with a slow backend) |

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
| `repo/handle.rs` `Writer` | Unit of work (lock guard) | the only way to put or delete an object; holds the write lock |
| `repo/nesting.rs`, `repo/parent_requests.rs` | Specification + State | the no-cycle rule on the in-memory graph; request, approve, decline |
| `repo/provide.rs` | Adapter | trait methods call the inherent ones |
| `mirror/sync.rs` | Template method | fixed flow: remember intent, read, plan, apply, remove, tidy the ledger |
| `right_flow.rs` | Strategy (pure) | the one place that says what up and down mean (provisional): effect clause (`reach`, `sources`), recipient clause (`recipients`), `directory_expresses` |
| `mirror/plan.rs` | Specification (pure) | create / update / keep / remove decided without I/O |
| `repo/mirror.rs` | Repository | wanted state + the ledger of mirrored ids |

## Decisions (the owner may change them)

* **Nesting has one home: `parents`.** The contract allows `Member::group` in `Memberships.add` too, which would be a
  second way to say the same thing (and a second place a cycle could hide). `add` refuses a group member and points to
  `request_parent` (then `approve_parent`). `Member::Group` stays for delegation holders.
* **Group content** is the AS2 `Group` plus the members `fs:kind` and `fs:parents` (namespace `https://freesynergy.net/ns#`).
  Index fields `kind` and `parent` are derived from it. Memberships, invitations and delegations are read back from their
  index fields (`group`, `member`, `role`, `state`, ...); the AS2 content is the standard-shaped view.
* **Effective membership** lists each person once, with the shortest path (`[asked group]` when direct). All paths are not listed.
* **Invitation** has the envelope type `Invite` (AS2 activity), expiry in the envelope `endTime`. `expires = None` never runs out. The
  time is always passed in. `expire_due(now)` is an addition to the contract (a sweep). Answering after the time: stored as `expired`,
  error `Expired`. `decline` and `revoke` ignore the expiry.
* **Right flow (PROVISIONAL, the safe reading; all of it is in `right_flow.rs`, change it there only).** The owner is still
  answering what up and down mean. Until then nothing is given away unless every group involved agrees. Two clauses, each
  changeable alone: **effect** (on which groups a right placed on a group applies) and **recipients** (whose people receive
  a right held by a group).
  *Effect:* `None` (default): the group only. `Down` / `Up`: also the children / parents, and onward, **but only groups that
  pass the same way**. Both ends of a link must agree, so a child that sets `Up` cannot pull a right from a parent that set
  `None` (or `Down`), and a parent cannot push one onto a child that set `None`. (A looser reading, "the target is not
  `None`", is a one-line change in `reach` and `sources`.) *Recipients:* a holder group's right reaches its **direct**
  members only, whatever anybody set; so a board that passes down does not reach an intern in a child, and a new parent of
  a holder does not capture its rights. A separate, explicit setting on the holder could widen that later; it would change
  `recipients` only. Membership (`effective`, `groups_of`, `is_member`) is about belonging and does not use the setting.
  `Manage` implies the other rights.
* **`may_do` in doubt says no.** Unknown group: `NotFound`. A group on a loop (or between loops), a holder group that is gone,
  a delegation or membership that cannot be read, an unknown right, an unknown setting (read as `none`): never a yes. A
  failing read is an error. The graph is loaded once per call.
* **Nesting needs the parent's consent (PROVISIONAL, who consents is the owner's to answer).** `request_parent` writes a
  `ParentRequest`; the link exists only after `approve_parent` (`decline_parent` ends the request). The loop rule is checked
  at both steps under the lock; the mirror and every graph question see approved links only. Who may approve: the
  repository's `actor` if it holds `manage` on the parent by `may_do`, or, while the parent has no delegation at all, if it
  created the parent. **Not enforced:** `actor` is just the string the `GroupRepo` was built with; nothing here authenticates
  it, `grant` is not guarded (whoever can call it can give themselves `manage`), and anybody may ask or end a link
  (`remove_parent`). The server in front has to supply the acting person and guard those calls.
* **Accept is for the invitee only.** `accept` needs `person` to be the invitee exactly (`NotInvitee` otherwise, the
  invitation stays pending). An e-mail invitee cannot be compared with a person's IRI here, so **provisionally**
  `accept_proved(id, person, proved_address, now)` takes the verified address the caller says the person proved and
  compares it with the invitee ignoring case. Nothing is verified in this crate; whoever calls it vouches. A secret token in
  the invitation is the alternative (more binding, not built). Both are additions to the contract (`acceptProved @5`).
* **Concurrency.** Every write that reads first runs under one lock through a `Writer` (the store handle is private to
  `handle.rs`, so a method that skips the lock does not compile). `add` and `grant` are idempotent on the same triple,
  invitation steps are atomic (one accept wins; a revoke that returned ok is never followed by `Accepted`). The lock is
  process wide, keyed by the identity of the store handle, so separate `GroupRepo`s over one handle share it. **Single-writer
  limit:** it does not cover a second process, nor a second handle on the same database file; the store has no
  compare-and-swap, so such writers can still store a cycle or lose an update. Keep to one writer per database. Walks stay safe
  on cyclic data.
* **Delegation**: see right flow for who is reached. `Delegation` has the store type `Delegation` (AS2 has none).
* **Mirror and right flow**: in Kanidm a member of a child is always `memberof` the parent, per group not switchable, so it
  gets what the parent holds. `directory_expresses` is true when `may_do` gives the same (the child is among the parent's
  `recipients`); with the safe reading it never is, so `SyncReport::divergences` names **every** nested edge. The effect clause
  has no counterpart in a directory and is not compared. The mirror does not adapt. Directory behaviour is unchanged.
* **Mirror**: the ledger (which ids were put into which directory, keyed by `GroupDirectory::directory_id`, default the provider
  name) is a store object of type `MirrorLedger`. The ids are written **before** the directory is touched (a failure later
  loses nothing) and tidied after; an id leaves only when its group was removed or the directory confirms (`read`) it is not
  there, so a group `list` left out once is not forgotten. Two directories of one provider need different `directory_id`s:
  **`fs-identity` must override it** (for example the Kanidm URL). Stored groups that cannot be read are listed in
  `SyncReport::unreadable` and never removed; a missing `fs:kind` is tolerated. A parent is applied only with the children the
  directory holds (`unnested` lists the rest). A directory that cannot map a person applies the rest and answers
  `Error::Unmapped(people)` (`unmapped` in the report); **`fs-identity` must do that in `convert.rs` `to_attrs`** instead of
  failing the whole group. Roles are not mirrored (a person once per group). Groups are applied children first.
* The whole graph is loaded once per `may_do` / nesting / effective question (one query). Measured in release on a chain of 1000
  nested groups (`examples/scale.rs`): building it takes about 18 s (a graph load per link), `may_do` for a stranger about 20 ms.
  An index of the closure is the next step if that is not enough.
* Empty `kind` and `role` are allowed; the store refuses empty index values, so they are just not indexed.


## Open points

* **Enforcement of rights** (`may_do` is not asked by `add`/`remove`/`delete`): needs the acting person on each call; comes with the capnp server.
* **`group.subscribe`** and the **capnp-rpc server** are not written. The traits return typed values (`Group`, ...), not `StoredObject`.
* The store's invitation-reply activities (`put_and_log` with `Accept` / `Reject`) are not used yet.

## Crates

`fs-store` (git tag v0.2.0), `fs-interfaces` (git tag v0.19.0), `serde_json`, `chrono`, `thiserror`, `async-trait`, `tokio` (sync only; dev: full).
All MIT / Apache-2.0. No Kanidm dependency of any kind.
