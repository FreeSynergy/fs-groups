//! Groups in one go: nest, add people, invite, ask for rights, refuse a loop.
//!
//! Run with `cargo run -p fs-groups-core --example demo`.

use std::sync::Arc;

use fs_groups_core::{Delegations, Error, GroupRepo, Invitations, Member, Right};
use fs_store::{Store, Timestamp, TursoStore};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let repo =
        GroupRepo::new(store, "https://example.org/users/owner").with_instance("example.org");

    let dept = repo.create("Energy department", "department").await?;
    let team = repo.create("Solar roofs", "project").await?;
    // Nesting needs the parent's consent: ask, then approve (as the creator of the department).
    let request = repo.request_parent(team.id, dept.id).await?;
    repo.approve_parent(request.id).await?;

    let ada = "https://example.org/people/ada".to_string();
    repo.add(team.id, Member::Person(ada.clone()), "member")
        .await?;
    println!(
        "ada is in the department: {}",
        repo.is_member(dept.id, &ada).await?
    );
    for m in repo.effective(dept.id).await? {
        println!("effective: {} via {} group(s)", m.person, m.via.len());
    }

    let expires = Timestamp::from_timestamp(4_000_000_000, 0);
    let invitation = repo
        .invite(team.id, "bo@example.org", "member", expires)
        .await?;
    let bo = "https://example.org/people/bo";
    Invitations::accept_proved(
        &repo,
        invitation.id,
        bo,
        "bo@example.org",
        Timestamp::from_timestamp(1_800_000_000, 0).unwrap(),
    )
    .await?;

    repo.grant(dept.id, Member::Person(ada.clone()), Right::Manage)
        .await?;
    println!(
        "ada may add members: {}",
        Delegations::may_do(&repo, dept.id, &ada, Right::AddMembers).await?
    );

    match repo.request_parent(dept.id, team.id).await {
        Err(e @ Error::Cycle { .. }) => println!("{e}"),
        other => println!("unexpected: {other:?}"),
    }
    Ok(())
}
