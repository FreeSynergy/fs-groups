# FreeSynergy Groups — belong together

**Who belongs to what, and who may do what, written down once and answered the same way everywhere.**

FreeSynergy Groups keeps groups of people: a family, a team, a club, a department that sits inside a bigger one. You can invite people, and an invitation can be accepted only by the person invited. Rights can be delegated to a member of a group.

What is different here: groups work alone, with no directory server needed. If you run a directory such as Kanidm, Groups can mirror its state into it, but the directory never decides. Groups can nest, and a group can have several parents, with a loop refused and the path named. Membership counts through nesting, and the answer says how a person got in. AI agents are never administrators of a group.

## What it can do today

Latest tag: `v0.3.0` (the code on main is already at 0.2.0).

- Create, rename, list and delete groups; nest them; ask a parent to accept a child.
- Add members with a role; ask "is this person in this group, and through which path?".
- Invitations with expiry; accepting makes the membership for the invited person only.
- Delegate and revoke rights, and ask "may this person do this?".
- Mirror groups to a directory behind the `GroupDirectory` interface; the Kanidm adapter is in [fs-identity](https://github.com/FreeSynergy/fs-identity). The mirror can be repeated safely, repairs manual changes and removes only what it created.
- Tested with the real in-memory store and forced interleavings.

## What is not there yet

- No window, no server, and no live change feed.
- Delegated rights only answer questions. Nothing else enforces them yet.
- No authentication yet.
- Several rules are provisional while the owner decides them: what "up" and "down" mean for rights between a group and its parent, and who must agree to a nesting link.
- One writer per store at a time.
- The interface is a draft with no consumer yet.

## Part of FreeSynergy

Groups decides who belongs together. Surveys already use it to widen a poll from a small group to its parent group, and the plan has the other programs ask it before they share. FreeSynergy is a family of programs that are free for everyone, forever, and that keep your data yours. The documentation, the manifesto and the overall plan live in [fs-documentation](https://github.com/FreeSynergy/fs-documentation); the reading version is at <https://freesynergy.github.io/fs-documentation/>. Start with the [manifesto](https://freesynergy.github.io/fs-documentation/manifesto/) if you want to know why we do this.


## For developers

- Workspace crate: `fs-groups-core` (core, no GUI). Start with `GroupRepo` and the traits `Groups`, `Memberships`, `Invitations`, `Delegations`, `GroupDirectory`.
- The contract is `groups.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (draft). Groups owns it.
- Build and test: `cargo build --workspace` and `cargo test --workspace`.
- Design and the full state table: [DESIGN.md](DESIGN.md).
- Licence: MIT OR Apache-2.0.
