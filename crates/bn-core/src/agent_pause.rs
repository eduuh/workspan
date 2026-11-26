//! A machine-wide switch that stops workspan from starting paid agent sessions.
//!
//! Every agent workspan starts on its own (dispatch, plan reviewers, task workers, review and
//! attention windows, pipeline stages, scheduled agents) costs tokens. `workspan agent pause`
//! drops a flag file in the state directory; every launch path checks [`refusal`] and refuses
//! while it exists. A harness you start yourself in a terminal never passes through workspan's
//! launch paths, so it keeps working.

use std::path::PathBuf;

const FLAG_FILE: &str = "agents-paused";

fn flag_path() -> PathBuf {
    crate::resolver::state_dir().join(FLAG_FILE)
}

/// The reason agents are paused, or `None` when they may run. An empty flag file still pauses.
pub fn paused() -> Option<String> {
    std::fs::read_to_string(flag_path())
        .ok()
        .map(|reason| reason.trim().to_string())
}

/// Pause agent launches on this machine, recording why.
pub fn pause(reason: &str) -> std::io::Result<()> {
    let path = flag_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, reason.trim())
}

/// Allow agent launches again. Returns whether they were paused.
pub fn unpause() -> std::io::Result<bool> {
    match std::fs::remove_file(flag_path()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// The message a launch path prints when it refuses, or `None` when agents may run.
pub fn refusal() -> Option<String> {
    paused().map(|reason| {
        let why = if reason.is_empty() {
            String::new()
        } else {
            format!(" ({reason})")
        };
        format!("agents are paused{why}; run `workspan agent unpause` to allow launches")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_refuses_until_unpaused() {
        crate::test_support::with_state_dir_path(|_| {
            assert_eq!(refusal(), None);
            pause("saving tokens").unwrap();
            assert_eq!(paused().as_deref(), Some("saving tokens"));
            assert!(refusal().unwrap().contains("(saving tokens)"));
            assert!(unpause().unwrap());
            assert_eq!(refusal(), None);
            assert!(!unpause().unwrap());
        });
    }
}
