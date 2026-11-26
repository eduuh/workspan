//! Reclaiming a worktree — **the** way it happens, and the one place that decides how.
//!
//! There used to be several. `workspan wt cleanup` ran the gates and closed the note; `workspan wt remove`
//! (`prefix D`) ran a bare `git worktree remove` past all of them, which on a `_pool/<n>` slot tore
//! the checkout out from under a live lease and left the pool permanently one slot short; and
//! `workspan wt close` (`prefix x`) had its own copy of the pool-release branch. Three ways to give a
//! worktree back, agreeing on nothing, and only one of them checked whether it was safe.
//!
//! So: one implementation, reached two ways, and nothing else.
//!
//! * [`super::cleanup_one`] is the implementation — the gates, the note close, the removal, the
//!   pool-lease hand-back. Every caller that reclaims a worktree goes through it.
//! * [`run_now`] is that same work, re-entered as `workspan wt close --reclaim` from the keystroke's own
//!   backgrounded process. Nothing a person does by hand in tmux should wait on a background
//!   scheduler — `run-shell -b` already keeps the UI free, so the keystroke can afford to do the
//!   work itself.

use super::*;

/// The worktree root a reclaim acts on: a pool slot's root, or the path itself.
///
/// A pane can sit anywhere inside a checkout, and for a slot it is the *slot* that gets handed
/// back, not the subdirectory you happened to be in.
pub(crate) fn target_root(wt_path: &Path) -> PathBuf {
    crate::wt_pool::pool_slot_root(wt_path).unwrap_or_else(|| wt_path.to_path_buf())
}

/// The label a reclaim is filed under: the branch, or — for a detached checkout, which has no
/// branch name — its directory. The same fallback [`super::cleanup_one`] makes.
pub(crate) fn label_for(wt_path: &Path, branch: &str) -> String {
    if !branch.is_empty() {
        return branch.to_string();
    }
    wt_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "worktree".to_string())
}

/// The shell command a reclaim runs — this same binary, re-entered on its `--reclaim` half.
/// Spelled as a CLI invocation rather than an in-process call so it is the exact string a person
/// can run by hand when the automatic one fails, and so the reclaim re-derives its facts (dirty
/// files, a live agent) from a tree whose windows are already gone.
///
/// `--confirmed` is carried across verbatim: the reclaim re-derives its plan from
/// [`super::close::close_plan`] when it runs, and that answer must reflect what the human actually
/// authorised, not what the tree looks like by then.
fn reclaim_cmd(path: &Path, confirmed: bool) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "workspan".to_string());
    let mut cmd = format!(
        "{} wt close --reclaim --path {}",
        shq(&exe),
        shq(&path.to_string_lossy())
    );
    if confirmed {
        cmd.push_str(" --confirmed");
    }
    cmd
}

fn run_reclaim(cmd: &str) -> bool {
    std::process::Command::new("/bin/sh")
        .arg("-lc")
        .arg(cmd)
        .status()
        .is_ok_and(|s| s.success())
}

/// Wait, briefly, for tmux to have reaped every pane rooted in `root`.
///
/// Slot occupancy is read from live panes, so a reclaim that runs before the killed window is gone
/// is refused as "slot in use". `kill-window` is synchronous, so this normally returns at once; the
/// bound is insurance, not an expected wait.
fn wait_for_windows_gone(root: &Path, branch: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !tmux_windows_for(root, branch).is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Reclaim the worktree now, in this process. Returns whether it worked, and the one-line outcome
/// for the caller to flash.
pub(crate) fn run_now(
    repo: &str,
    label: &str,
    branch: &str,
    path: &Path,
    confirmed: bool,
) -> (bool, String) {
    wait_for_windows_gone(path, branch);
    let cmd = reclaim_cmd(path, confirmed);
    match run_reclaim(&cmd) {
        true => (true, format!("closed {repo}/{label} — reclaimed")),
        false => (
            false,
            format!(
                "closed {repo}/{label} — the reclaim failed; run `workspan wt close --reclaim --path {}`",
                path.display()
            ),
        ),
    }
}

/// Rebuild a pool slot that has just been handed back, so the next lease lands on a warm checkout
/// instead of one whose build is behind the base the reset moved it to. Returns the one-line
/// outcome to flash.
///
/// Yields to a pool warm already running: that warm builds the idle slots itself, and two builds
/// in one checkout would race. Holds the warm lock for the build for the same reason.
pub(crate) fn rebuild_slot(repo: &str, slot: &Path) -> String {
    let n = slot
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(_warm) = slot.parent().and_then(crate::wt_pool::slot_lock::try_lock_warm) else {
        return format!("{repo} slot {n}: a pool warm is running — it will build the slot");
    };
    match crate::wt_pool::warm::build_one_slot(repo, slot) {
        true => format!("{repo} slot {n}: rebuilt — warm for the next lease"),
        false => format!(
            "{repo} slot {n}: rebuild failed — run `workspan wt pool warm --repo {repo} --slot {n}`"
        ),
    }
}

#[cfg(test)]
#[path = "reclaim_tests.rs"]
mod reclaim_tests;
