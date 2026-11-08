# fs-groups – Design

`fs-groups` is the groups program of FreeSynergy. This repo holds the **program core** (`crates/fs-groups-core`, no GUI) and its **Cap'n Proto server** (`crates/fs-groups-server`).
A group is an **AS2 `Group`** in an envelope stored through `fs-objects` (ADR 0014, 0020, 0050). It runs alone, without any
directory (ADR 0024); a directory is only a mirror (ADR 0047). Contract: `groups.capnp` in `fs-interfaces` v0.46.0 (draft).

## State

Version 0.4.0. The server also serves `Follows` (`follow.deliver`, `follow.read`, `follow.receive` are listed in `capabilities()`). **`sender` of `receive`/`withdraw` is named by the client; nothing authenticates it** (same limit as the rest of the transport). The server wires `OperatorIsHuman`: the `--actor` is a person, everybody else has no answer; `fs-groups-people` replaces this once a people provider is reachable. **Several rules below are provisional: the owner is still answering what up and down mean and who consents to a nesting link.** A tag is fine as long as these points stay marked provisional.

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
| Mirror option B (ADR 0056): nested only where child and parent both pass `Up` and nothing forbids it, else a flat list (the child's whole subtree); old nested form healed; `divergences` = **every** edge (the nested ones too) | done, **provisional** |
| Agents never admin: `AiMarking` (injected), admin roles by configuration (folded, ASCII), installer by configuration, identities in NFC and checked by shape; no answer = refuse; in the mirror one invariant on the final plan (`enforce`), checked on every prefix of the writes | done, **provisional, no authentication** |
| Mirror hardening: ledger written before the directory (intent), tidied only when it changed; keyed by `directory_id`; everything is read once and sorted by type, index and content must agree, anything unreadable fails closed and is reported (not quiet); revocations elsewhere go on; loops are never nested; a parent is applied only with the children the directory holds; one unmapped person does not block the group | done |
| One write path (`Writer` holds the lock), process-wide lock per store handle | done; **single-writer limit, see Decisions** |
| Following: `Follows` (deliver, pass_on, revoke_delivery, deliveries, receive, withdraw, sources, remove_source); contract `Follows` in `groups.capnp` (fs-interfaces v0.38.0, provisional) | done in the core, **provisional**; relayed copies keep `origin`; the server serves it and lists the `follow.*` capability ids by `capabilities()` (the server does not serve `Follows`). **Not built:** the capnp server half, a transport that sends the offer, an authenticated sender and acting person |
| `group.subscribe`: `GroupFeed` (Observer) tells every committed write of a group, membership, invitation, delegation or parent request, with the envelope the store committed; `Deleted` carries the id only | done, draft; **what the feed covers is provisional** |
| `Group.envelope`: the group as stored, as the contract's `Group` has it | done |
| Server `fs-groups-server` (writes of several steps finish even if the client goes away): every method of `Items`, `Memberships`, `Invitations`, `Delegations`, `Follows`, `subscribe`, `supportReport`; `addParent` answers `Unsupported`; typed problems `groups/{not-found 404, cycle 409, conflict 409, invalid-transition 409, not-invitee 403, forbidden 403, expired 410, directory 502, unmapped 502}`, shared `invalid-request` 422 and `too-large` 413; binary with the shared `ServerOptions` | done, draft; **no authenticated caller, see the note below** |
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
| `traits/ai.rs`, `repo/admins.rs` | Strategy (injected) + Specification | the host says whether a member is an agent; one place says what counts as admin |
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
| `mirror/sync.rs`, `mirror/steps.rs` | Template method | fixed flow: remember intent, read, plan, enforce, write in order, remove, tidy the ledger |
| `mirror/enforce.rs` | Specification (pure) | the one place that knows the rule about agents, applied to the final plan |
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
  `None`", is a one-line change in `reach` and `sources`.) *Recipients:* a holder group's right reaches its **direct** members only, in every setting, whatever anybody set. So a board
  that passes down or up does not reach an intern in a child, an `Up` holder does not reach the members of an `Up` child (the
  holder was never asked, and `set_right_flow` is unguarded, so a child could make itself a recipient), and a new parent of a
  holder does not capture its rights. This is the safe reading and it **stands until the owner answers** (ADR 0056: splitting
  into two settings, consent on both sides; open, not built). Widening it would change `recipients` only. Membership
  (`effective`, `groups_of`, `is_member`) is about belonging and does not use the setting.
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
* **Mirror, option B (ADR 0056, provisional).** In Kanidm rights always go from the child upward (a member of a child gets what
  the parent holds); that holds only there and cannot be set per group. A nested edge goes to the directory only where the
  group is meant to pass on (`right_flow::directory_nests`: child and parent both pass `Up`) and nothing forbids it (a loop, or
  an administrative parent and an agent in the child's closure, see below). Every other child is not nested; the parent's
  **flat list** holds its own direct people plus the people of the child's whole subtree. So the directory's closure of a group
  equals `effective`, except that a group carrying an administrative right never lists an agent or an unknown person, so `effective` may name someone (an agent in an administrative group, or in a child below it) whom the directory does not list. Nobody else who belongs loses anything, and a person may be listed in a group directly because of a child.
  A change of setting or an edge removed is healed by the normal sync.
  **What remains in `SyncReport::divergences`: every edge, the nested ones too.** `may_do` gives a holder's right to direct
  members only, while the directory gives it to every member of a child, nested or listed flat. This over-grant is **open with
  the owner**; it is not built away and `may_do` does not follow Kanidm's nesting. The effect clause has no counterpart in a
  directory and is not compared.
* **Agents are never admin (ADR 0056, provisional).** This crate does not depend on fs-people: the host injects `AiMarking`
  (`with_ai_marking`; the bridge is the crate `fs-groups-people`, `PeopleMarking`, which asks the role book and decides with `fs_people_core::is_ai_role`, so the short id `ai` and the full IRI both count; a person with no role, or an identity that is no `…/people/<id>`, gets no answer). Admin is `Right::Manage` plus the role ids of `with_admin_roles`. `grant` of
  `Manage` to a person and `add` (also through `accept`) of an admin role are refused for an agent (`AgentNotAdmin`) and **when
  there is no answer or no checker** (`AiStatusUnknown`); a failing checker is an error. Through a holder group: a group
  containing an agent **may** hold `Manage`, but `may_do` never counts a `Manage` delegation for a person who is an agent or not
  known to be a human (so the agent keeps what is below manage, and also loses `Manage` if marked later). An agent also cannot
  answer a parent request. This is only an answer: **nothing verifies who calls**, and a caller that wires no checker or a
  checker that always says human defeats it.
  **Spelling and shape.** Role ids and the configured admin roles are compared folded (trimmed, case folded), so `" Owner "` and `OWNER`
  are the admin role. **Role ids are ASCII only** (letters, digits and `- _ . : / # @ + ~ % =`): that refuses control, invisible and
  look-alike characters by shape (a Cyrillic `о` in `оwner`), the smaller of the two options, at the price that a role id with an
  umlaut is refused here. Persons, invitees, holders and the installer are checked in Unicode normal form C (a decomposed and a composed spelling are one
  person) and are made of letters and digits of any script, combining marks (a virama, a diaeresis), the ASCII punctuation of an
  IRI or an e-mail local part (including `^ { | }`, backtick and quotes), and a zero width (non-)joiner between letters or after a
  mark (Persian, Devanagari). **Still refused:** whitespace, controls, every other invisible or format character (all default
  ignorable code points such as the Hangul fillers, bidi controls, variation selectors, tags), a joiner at an end or between
  non-letters, and unassigned code points; look-alike letters of other scripts are not told apart (the host's identity rules). Configuration that breaks this, an empty admin role, or an empty or padded installer is
  refused. A role id that is not an admin role stays an opaque string. **Without `with_admin_roles` no role counts as admin, so an
  agent can be added with the role `owner`: naming the admin roles is the host's duty.** The person is asked of the host with
  scheme and host in lower case and nothing else changed (the user part, path, query, fragment and a trailing slash are different
  identities). **`AiMarking` must answer `None` unless it knows the person**: `Some(false)` means "known to be human"; a thin
  wrapper over `RoleBook::is_ai` (false for an id with no roles) turns unknown into human, see the trait doc for the adapter.
  An agent may hold `AddMembers` or `RemoveMembers` (only `Manage` and admin roles are barred): ADR 0056 leaves that open.
  **In force.** `Membership::in_force` and `Delegation::in_force` are `false` for an admin role or `Manage` held by a person who
  is (now) an agent or not known to be human: the entry stays, but counts for nothing. An admin *role* alone grants no right
  (`may_do` stays false): it only bars agents; appoint an admin by granting `Manage`.
  **In the mirror: one invariant, checked on the final plan.** A directory group *carries an administrative right* when it holds
  `Manage` on anything or a member has an admin role in it. The invariant: **no agent and no unknown person is in the closure of
  such a group**, whether listed in it, listed flat from a child, nested through a child, wanted, held still or kept as it is.
  It is no longer kept by checks along each path. `sync_to` works in three steps:
  1. *Plan.* For every group the directory will hold after the run (the groups we write, held still where something unreadable
     touches them, and the ones we leave as they are, such as a group we cannot read) the members and nested edges it will
     have. The directory's own list is only an input.
  2. `mirror::enforce(plan, rule, verdicts)`, a pure function and the only place that knows the rule: it walks the planned
     closure of every administrative group; whoever the rule bars is dropped from a direct listing, and a nested edge whose
     child closure holds one is replaced by a flat listing without them. It repeats until stable and names a person in
     `SyncReport::left_out` only if they are really out of that group's closure. The host's verdicts (`Mirror::verdicts`, asked
     for everybody in the plan, also people the directory lists that we cannot name from our data) are its input; a person
     without a verdict is unknown. A group the rule changes but we do not otherwise write (an agent in a group we cannot read)
     is written too.
  3. *Order.* Writes that take away go first (parents before children by the nesting the directory has now), then those that
     add (children first). A write that adds anybody is held back, and reported, when a group above it in the directory
     could not be updated, or when it adds an edge to a group that is not yet in its final state; a group that is held back or
     refused blocks the adds below it in turn. So the rule holds after every write, refused ones included
     (`tests/mirror_property.rs`: random states, refusals and unreadable objects; no new violation after any prefix of the
     writes, and none at the end when nothing is refused).
  **Strict and lenient.** A group is *known* administrative when a readable delegation or role says so, or when an unreadable
  object that still names it (a membership of it, a delegation held by it, in the index or the content) might. There agents
  and unknown people are barred. If an unreadable delegation might be `Manage` and its holder cannot be told at all, **any group
  may be administrative**: then only known agents are barred everywhere, and unknown people are reported
  (`SyncReport::unchecked`) but stay, so a host that cannot answer does not empty ordinary groups. No checker wired: everyone is
  unknown, so a known administrative group gets no people.
  **What counts as unreadable (fail closed).** Everything in the context is read once and sorted by type. Unreadable are: a
  group or membership or delegation that cannot be parsed; one whose index and AS2 content disagree (the content is authoritative:
  group, person, role, right; a hand-edited `right=read` over `manage` is noticed); an object of a type we do not know (a
  membership changed to `Note`); a membership of a group; a group with a corrupt parent entry (`fs:parents` not a list of ids)
  or a parent that exists nowhere. They are listed in `SyncReport::unreadable` and the run is not quiet. Where one touches a
  group (the group, the groups above it, or every group if that cannot be told) the sync keeps what the directory holds (and
  the rule still applies to it); revocations elsewhere go on. A stored object whose type changed is never removed from the
  directory.
* **The top admin is whoever installs (provisional).** `with_installer(identity)` is configuration only: `may_do` is true for that
  identity on every group that is not on a loop, if they are known not to be an agent. They appoint further admins by `grant`ing `Manage` (or an admin
  role); who may grant is not enforced. No authentication is built.
* **Mirror**: the ledger (which ids were put into which directory, keyed by `GroupDirectory::directory_id`, default the provider
  name) is a store object of type `MirrorLedger`. The ids are written **before** the directory is touched (a failure later
  loses nothing) and tidied after; an id leaves only when its group was removed or the directory confirms (`read`) it is not
  there, so a group `list` left out once is not forgotten. Two directories of one provider need different `directory_id`s:
  **`fs-identity` must override it** (for example the Kanidm URL). Stored groups, memberships and delegations that cannot be read are listed in `SyncReport::unreadable` (not
  quiet); where they touch a group nobody is taken out of it (what is wanted may be short; see agents) and an unreadable group
  or membership can still be deleted. Groups on a loop (bad data from a second writer) are in `looped` and never nested. A
  repeat run does not rewrite the ledger. A missing `fs:kind` is tolerated. A parent is applied only with the children the
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
* **`Follows` (`Groups.follows @6`) is served** (`follows_service.rs`) and `supportReport` lists `follow.deliver`, `follow.read` and `follow.receive`; `errors.rs` pins the list. `revoke`, `removeSource` and `withdraw` ask no acting person (`deliver` and `passOn` do): like every write here they are as safe as "same system user", and a client that can name a delivery id can end it.
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
* **Two settings and consent on both sides** (open with the owner, ADR 0056): the safe reading stays until he answers.
* **`group.subscribe`** and the **capnp-rpc server** are not written. The traits return typed values (`Group`, ...), not `StoredObject`.
* The store's invitation-reply activities (`put_and_log` with `Accept` / `Reject`) are not used yet.

## Crates

Core: `fs-objects` (git tag v0.5.0), `fs-interfaces` (git tag v0.46.0), `serde_json`, `chrono`, `thiserror`, `async-trait`, `tokio` (sync only; dev: full). Server: also `fs-interfaces` feature `rpc`, `capnp`, `capnp-rpc`; dev: `tempfile`, `uuid`.
All MIT / Apache-2.0. No Kanidm dependency of any kind.

## One binary `fs-groups` and its window (ADR 0063, 2026-11-08)

`crates/fs-groups` is the only binary: `fs-groups` without arguments opens the window, `fs-groups serve` runs the server (`fs_groups_server::serve`), feature `gui` (default) builds the window. `fs-groups-server` and `fs-groups-app` are libraries. The window (`crates/fs-groups-app`) is a frontend over `groups.capnp`: `GroupsApi` (trait first) with `GroupsRemote` (a worker thread owns the Cap'n Proto connection) and `GroupsAdapter`, which gives the groups to the list-and-page workbench of `fs-components` (built for people, neutral in what it shows) as rows (name, kind), a sheet (kind, what the group is part of, its direct members) and a form (name, kind). If no server answers on the user's own socket the window starts `fs-groups serve` detached. Tests: `tests/remote.rs` (real server, real store), `crates/fs-groups/tests/binary.rs` (the real binary).

Not in the window yet (nothing pretends otherwise): nesting (requests, approval), adding and removing members, invitations, delegations, rights, follows, mirrors; no revision on the wire, so a change by someone else is not detected on save; no timeout waiting for the server.
