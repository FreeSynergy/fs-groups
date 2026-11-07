# fs-groups – Design

`fs-groups` is the groups program of FreeSynergy. This repo holds the **program core** (`crates/fs-groups-core`, no GUI) and its **Cap'n Proto server** (`crates/fs-groups-server`).
A group is an **AS2 `Group`** in an envelope stored through `fs-store` (ADR 0014, 0020, 0050). It runs alone, without any
directory (ADR 0024); a directory is only a mirror (ADR 0047). Contract: `groups.capnp` in `fs-interfaces` (draft; **temporarily the branch `groups-members-cursor`**, until `run_to_end` and the `Memberships` cursor are tagged, then pin the tag).

## State

Version 0.3.0. **Several rules below are provisional: the owner is still answering what up and down mean and who consents to a nesting link.** No tag of this repo until he has.

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
| Following: `Follows` (deliver, pass_on, revoke_delivery, deliveries, receive, withdraw, sources, remove_source); contract `Follows` in `groups.capnp` (fs-interfaces v0.38.0, provisional) | done in the core, **provisional**; relayed copies keep `origin`; the `follow.*` capability ids are not listed by `capabilities()` (the server does not serve `Follows`). **Not built:** the capnp server half, a transport that sends the offer, an authenticated sender and acting person |
| `group.subscribe`: `GroupFeed` (Observer) tells every committed write of a group, membership, invitation, delegation or parent request, with the envelope the store committed; `Deleted` carries the id only | done, draft; **what the feed covers is provisional** |
| `Group.envelope`: the group as stored, as the contract's `Group` has it | done |
| Server `fs-groups-server` (writes of several steps finish even if the client goes away): every method of `Items`, `Memberships`, `Invitations`, `Delegations`, `subscribe`, `supportReport`; `addParent` answers `Unsupported`; typed problems `groups/{not-found 404, cycle 409, conflict 409, invalid-transition 409, not-invitee 403, forbidden 403, expired 410, directory 502, unmapped 502}`, shared `invalid-request` 422 and `too-large` 413; binary with the shared `ServerOptions` | done, draft; **no authenticated caller, see the note below** |
| Tests | core: `tests/*.rs` by feature, real in-memory `TursoStore`, forced interleavings with a slow backend. Server: over a real Unix socket against the store-backed provider, the real binary across restarts, exit codes and data directory rights |

> **The server does not know who is calling.** The transport only checks that the peer runs as the same system user
> (`SO_PEERCRED`). `--actor`, the approver of a parent request, `requester` in `ParentRequest`, `person` in `accept` and
> `mayDo`, and `provedAddress` in `acceptProved` are **plain strings**, never a verified identity, and must not be shown as one.
> Restarting the server with another `--actor` is all it takes to change who "may approve". A real check needs an
> authenticated caller in the transport and the contract; neither exists yet (see Open points).

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
| `traits/feed.rs` | Observer | `GroupFeed`, `GroupListener`, `Registry` (subject), `Subscription` (drop = unsubscribe) |
| `repo/notify.rs` | Decorator | wraps the store's `ObjectRepository`, so no write path can forget to tell the feed |
| `fs-groups-server/{groups,items,nesting,memberships,invitations,delegations}_service.rs` | Adapter + Factory | wire in, one trait call, wire out; depends on the traits only |
| `fs-groups-server/{wire/*,errors,feed}.rs` | Adapter, Observer | values on the wire with their limits, core errors as RFC 9457 problems, `GroupFeed` to the shared forwarder |
| `follow_rules.rs`, `repo/deliveries.rs`, `repo/sources.rs`, `repo/follow_codec.rs` | Specification (pure) + State + Repository | following between independent groups; not a tree, parents and cycles untouched. Repository turned round: the giver delivers, the receiver lists sources; State for delivery and source; Specification for "never more than the giver has", "pass on only with permission", "the sender is the origin" |
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


## Server decisions (provisional, the owner may change them)

* **`GroupDirectory` is not served here.** fs-groups is its client (`Mirror::sync_to`), `fs-identity` its provider. The contract's root
  `Groups` hands out no directory and has no method to start a mirror, so a mirror still runs only in-process.
* **`addParent @6`** answers `Unsupported` (capability id `group.addParent`, also in the report), as the contract says since v0.19.0.
* **`supportReport` is the standard; `capabilities` is not added to the `Groups` root** (strategy decision). `tasks.capnp` has it, this contract does not, so `Capabilities::write_list` is not used here.
* **Clock:** the core takes the time as an argument; the server reads it for `accept` and `acceptProved`. `Invitations.list` sweeps with `expire_due` first, so `expired` is true when read; a provider without the sweep still answers its list.
* **Feed:** all stored kinds, not only groups (the consumer tells by `envelope.type`), since a membership change has no other way to reach a consumer. The mirror ledger stays out. A feed belongs to one `GroupRepo` and its clones.
* **Text limits:** a name or kind: `check_intake` is asked about 16 times the length (a name and kind come back twice, a control character takes six bytes in JSON), about 1 MiB; a person, invitee, role or address: **4 KiB** (an IRI fits, so few entries cannot fill an answer). Both 413 before anything is stored. The limit is the server's: an in-process caller of the core has none.
* **Writes that finish:** every write of several steps (`delete`, `approveParent`, `accept`, `acceptProved`, the sweep in `Invitations.list`) reads its parameters and runs through `fs_interfaces::rpc::run_to_end`, so a client that cancels or disconnects cannot leave it half done. The store has a transaction per object only (`put_and_log`), none across objects, so this is not atomic: a **crash or store failure** in the middle still leaves a partial cascade. Calling `delete` or `approveParent` again completes it.
* **Lists:** `Items.list`, `Memberships.direct` and `effective` page by weight (the two were appended to the contract for this: the ids for `remove` only come from `direct`). The others (`invitations.list`, `delegations.list`, `parentRequests`, `groupsOf` and the id lists) still have no cursor and answer 413 rather than end the connection if they would not fit, about 8000 entries of the longest identifier. `invitations.list` and `delegations.list` can lock out `revoke` that way: the ids come from `invite` and `grant` too, but only to that caller.
* **`Items.list`** walks the provider's pages and cuts the wire page by weight, so each page reads the whole list; cursors are places in that list (`fs-interfaces/DESIGN.md`).
* **`--instance`** comes from the shared `ServerOptions` (since `fs-interfaces` v0.24.0); without it IRIs say `local.invalid`.
* **`Follows` (`Groups.follows @6`) is not served.** The core offers it (`GroupRepo`), the server hands out no `Follows`; the `follow.*` capability ids are therefore not in `supportReport` either (the report would claim what the server does not serve; `errors.rs` pins the list). Whoever builds the server half maps the trait and adds the ids.
* **Right flow:** the server maps the enum and nothing else; the meaning stays in `right_flow.rs` and may change there.

## Following (ADR 0056 "Einseitig folgen", owner 2026-11-07; the form is a proposal)

Independent groups, no tree. **Who allows access, delivers.** The receiver gets an offer (proposal: AS2 `Invite`), its server
enters the source without any confirmation, and the receiver can remove it again. Mutual following is allowed. Only the giver's
own rights go, never more than the giver holds (checked with `may_do`). A receiver may pass a delivery on to a third group only
if the source marked it so (flag on the delivery); the giver can revoke at any time. "Belongs to" with parents stays as it is
and following never touches it.
Order of work on branch `follow`: (1) pattern, (2) contract and trait, answering `Unsupported`, (3) tests against the trait,
(4) implementation. Assumptions (all provisional): `Manage` is never delivered to a foreign group; a relayed delivery is not
passable again; revoking or removing a source revokes the deliveries relayed from it; the host authenticates the sender.
Stored as `Delivery` (giver side, index `group` = giving group) and `Source` (receiver side, index `group` = receiving group), so
`delete` clears both without extra code. `receive` checks only that the offer's source lies at the origin the transport named
(`follow_rules::from_origin`): **nothing authenticates that origin**, so a host that passes any name defeats it. Without that, anybody
could enter themselves as a source of somebody else (the harm is a false entry, not a right: a right only ever lies on the giver's
own server). Entering without confirmation also means anybody can fill a group's source list; the receiver removes entries. Revoking
needs a message `withdraw` to the receiver; if it never arrives the entry stays `Active` there (the giver's own record is what counts).
`deliver` asks `may_do` of the acting person on the giving group; `pass_on` asks it on the passing group for every right passed on, so a
relay cannot give what its actor does not hold (the conservative reading, **provisional**). `revoke_delivery` and `remove_source` do not
ask who acts (same gap as nesting). **Origin (provisional):** `Offer.origin` / `Delivery.origin` name the group whose right it is; a
relayed copy keeps the first source while `source` says who sent it, and a copy is not passed back to its origin. Nothing proves that an
`origin` that differs from `source` is true: the receiver trusts its sender for it, as for everything else the sender says.
Ids are typed: a source id is `NotFound` for `revoke_delivery` and a delivery id for `remove_source`. Ending twice writes and tells nothing.

## Open points

* **Authenticated caller.** Nothing in the transport or the contract names one. So `may_do` is asked only for approving a parent request, about `--actor`; `add`, `remove`, `delete`, `grant` and `remove_parent` are not guarded, and every client of this user can do all of it. **This is not enforcement of rights.**
* **`acceptProved`** passes on an address the client vouches for; the provider verifies nothing. Only as safe as "same system user" for now.
* **Consent after a grant:** the creator may approve only while a parent has no delegation at all, so one `read` grant ends that, and then only a `manage` holder can approve (the core's rule, provisional).
* **Question for the owner: may a group hold a right on itself?** `grant` allows it today, and `delete` handles it. Unchanged until he answers.
* **Question for the owner: mail invitations (provisional).** Plain `accept` is refused for an e-mail invitee whatever the string, so saying the address is no proof; only `acceptProved` works, and there the client vouches for the address. Whether a secret token replaces it is open.
* The store's invitation-reply activities (`put_and_log` with `Accept` / `Reject`) are not used yet.

## Crates

Core: `fs-store` (git tag v0.2.0), `fs-interfaces` (temporarily branch `groups-members-cursor`, then a tag), `serde_json`, `chrono`, `thiserror`, `async-trait`, `tokio` (sync only; dev: full). Server: also `fs-interfaces` feature `rpc`, `capnp`, `capnp-rpc`; dev: `tempfile`, `uuid`.
All MIT / Apache-2.0. No Kanidm dependency of any kind.
