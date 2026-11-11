//! The version handshake of the window: no server and no start is a visible error on every call, never a hang or a
//! panic, and a later call connects when the server is there.
//!
//! Pattern: Integration test.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs_groups_app::{ApiError, GroupsApi, GroupsRemote};

#[test]
fn no_server_and_no_start_is_an_error_the_window_can_show() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
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
}
