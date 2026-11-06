//! Times a chain of nested groups: building it and asking `may_do` for a stranger.
//! `cargo run --release -p fs-groups-core --example scale -- 1000`

use std::sync::Arc;
use std::time::Instant;

use fs_groups_core::{Error, GroupRepo, Member, Right, RightFlow};
use fs_store::{Store, TursoStore};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let repo = GroupRepo::new(store, "https://example.org/users/owner");
    let started = Instant::now();
    let mut previous = None;
    let mut ids = vec![];
    for i in 0..n {
        let g = repo.create(&format!("g{i}"), "t").await?;
        repo.set_right_flow(g.id, RightFlow::Down).await?;
        if let Some(parent) = previous {
            let request = repo.request_parent(g.id, parent).await?;
            repo.approve_parent(request.id).await?;
        }
        previous = Some(g.id);
        ids.push(g.id);
    }
    println!("build {n} nested groups: {:?}", started.elapsed());
    repo.grant(
        ids[0],
        Member::Person("https://example.org/people/ada".into()),
        Right::Read,
    )
    .await?;
    let started = Instant::now();
    let seen = repo
        .may_do(
            ids[n - 1],
            "https://example.org/people/stranger",
            Right::Read,
        )
        .await?;
    println!(
        "may_do for a stranger on the deepest group ({seen}): {:?}",
        started.elapsed()
    );
    Ok(())
}
