//! A server of another version that cannot be replaced: the person gets the words of the handshake error with the
//! version it needed and why no new server came.
//!
//! Pattern: Integration test. Own file: the handshake stops the old server (`Control.stop`), one flag per process.

mod support;

use std::io;
use std::sync::Arc;

use fs_groups_app::{ApiError, GroupsApi, GroupsRemote};
use support::serve_as;

#[test]
fn an_old_server_that_cannot_be_replaced_is_reported_with_the_needed_version_and_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("groups.sock");
    serve_as(&socket, "0.0.1");
    let refuse = || -> io::Result<()> { Err(io::Error::other("cannot start here")) };
    let remote = GroupsRemote::matching(&socket, Arc::new(refuse));
    match remote.list() {
        Err(ApiError::Failed(text)) => {
            assert!(text.contains("cannot start here"), "why: {text}");
            assert!(
                text.contains(env!("CARGO_PKG_VERSION")),
                "what is needed: {text}"
            );
        }
        other => panic!("expected a handshake error, got {other:?}"),
    }
}
