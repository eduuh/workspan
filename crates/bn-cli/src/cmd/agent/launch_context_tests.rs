//! Split from `agent.rs` under the file-size ratchet.
use super::*;

#[test]
fn sessions_require_a_git_worktree() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = resolve_launch_context(&Config::default(), dir.path())
        .expect_err("a plain directory must not back a session");
    assert!(error.contains("sessions require a git worktree"));
}
