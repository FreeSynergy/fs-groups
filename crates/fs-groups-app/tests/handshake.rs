//! The version handshake of the window: a server of this version is used as it is, a missing one is started by the
//! starter, and when none fits the person gets the reason on every call, never a hang or a black window. Also the
//! refusal of a facet this program does not have (window and server).
//!
//! Pattern: Integration test. Nothing here stops a server: `Control.stop` is one flag per process, so the test that
//! does lives in its own file (`handshake_mismatch.rs`).
//!
//! Known limit (DESIGN.md): a server that goes away while the window runs is not reconnected by itself; the next call
//! after a failed handshake connects anew, but a lost connection reports "went away" until the window is reopened.

mod support;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs_groups_app::{ApiError, GroupsApi, GroupsRemote};
use support::serve_as;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn socket() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
    (dir, socket)
}

#[test]
fn a_server_of_this_version_is_used_as_it_is() {
    let (_dir, socket) = socket();
    serve_as(&socket, VERSION);
    let refuse = || -> io::Result<()> { Err(io::Error::other("must not be started")) };
    let remote = GroupsRemote::matching(&socket, Arc::new(refuse));
    assert!(remote.list().is_ok_and(|l| l.is_empty()));
}

#[test]
fn no_server_and_no_start_is_an_error_the_window_can_show() {
    let (_dir, socket) = socket();
    let tried = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&tried);
    let refuse = move || -> io::Result<()> {
        flag.store(true, Ordering::SeqCst);
        Err(io::Error::other("no executable"))
    };
    let remote = GroupsRemote::matching(&socket, Arc::new(refuse));
    assert!(tried.load(Ordering::SeqCst), "the starter was asked");
    match remote.list() {
        Err(ApiError::Failed(text)) => assert!(text.contains("no executable"), "{text}"),
        other => panic!("expected a visible failure, got {other:?}"),
    }
    assert!(remote.reconnect().is_err(), "still nothing there");
    // The person fixes it (here: the server is there now) and refreshes: the next call connects anew.
    serve_as(&socket, VERSION);
    assert!(remote.list().is_ok_and(|l| l.is_empty()));
}

#[test]
fn a_facet_this_program_does_not_have_is_refused() {
    let words = ["--facet".to_owned(), "nope".to_owned()];
    let err = fs_groups_app::gui::run(&words).unwrap_err();
    assert!(err.contains("nope"), "{err}");
    let served = fs_groups_server::serve(["--facet", "nope"].map(Into::into));
    assert!(matches!(
        served,
        Err(fs_groups_server::ServeError::Usage(m)) if m.contains("nope")
    ));
}
