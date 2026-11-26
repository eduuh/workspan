//! `reset-to-main` — force-remove every worktree of one repo, then bring its main up to date.
//!
//! The picker's one destructive verb, so it is split in two: [`plan`] is pure and decides what goes,
//! which lets the picker show the exact list in its confirmation prompt and lets the tests pin every
//! keep/remove rule; [`job_reset_to_main`] carries the plan out.
//!
//! What "every worktree" means, and what it deliberately does not:
//!
//! * The main checkout stays; it is what gets pulled.
//! * A *leased* pool slot is returned to the warm pool with `--force` rather than deleted, so its
//!   ignored deps and build cache survive. An idle slot is already at rest and is left alone.
//! * Every other worktree is `git worktree remove --force`d, dirty or not.
//! * Local branches are kept. Removing a checkout is the point; deleting the commits under it is not,
//!   so unpushed work stays reachable by branch name.
use crate::*;
use std::path::{Path, PathBuf};

/// What happens to one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResetStep {
    /// `git worktree remove --force` the checkout.
    Remove { path: PathBuf, label: String },
    /// Hand a leased pool slot back to the warm pool, discarding its changes.
    ReturnSlot { path: PathBuf, label: String },
}

impl ResetStep {
    pub(crate) fn describe(&self) -> String {
        match self {
            ResetStep::Remove { path, label } => format!("remove  {label}  ({})", path.display()),
            ResetStep::ReturnSlot { path, label } => {
                format!("return  {label}  ({}) to the warm pool", path.display())
            }
        }
    }
}

/// Decide what goes, from `git worktree list --porcelain`.
///
/// Pure, so every rule below has a test. `main_dir` is the repo's main checkout, `pool_dir` the
/// directory holding its slots, `default` the default branch name.
pub(crate) fn plan(porcelain: &str, main_dir: &Path, pool_dir: &Path, default: &str) -> Vec<ResetStep> {
    let main_slot = pool_dir.join(bn_core::agent::MAIN_POOL_SLOT.to_string());
    let mut steps = Vec::new();
    for entry in porcelain.split("\n\n") {
        let mut path = None;
        let mut branch = None;
        let mut skip = false;
        for line in entry.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p.trim()));
            } else if let Some(b) = line.strip_prefix("branch ") {
                branch = Some(b.trim().trim_start_matches("refs/heads/").to_string());
            } else if line == "bare" || line.starts_with("prunable") {
                // The bare repo itself, or a registration whose directory is already gone (the
                // closing `git worktree prune` drops those).
                skip = true;
            }
        }
        let Some(path) = path else { continue };
        if skip || path == main_dir || path == main_slot {
            continue;
        }
        let label = branch.clone().unwrap_or_else(|| "(detached)".to_string());
        if path.starts_with(pool_dir) {
            // A slot is leased when it holds a branch other than the default; a detached slot is
            // idle and already at rest.
            match branch.as_deref() {
                Some(b) if b != default => steps.push(ResetStep::ReturnSlot { path, label }),
                _ => {}
            }
        } else {
            steps.push(ResetStep::Remove { path, label });
        }
    }
    steps
}

/// The plan for `repo` as it stands on disk right now, or why there is none.
pub(crate) fn plan_for(cfg: &Config, repo: &str) -> Result<Vec<ResetStep>, String> {
    let (bare_dir, worktree_dir) = wt::layout(cfg);
    let bare = bare_dir.join(format!("{repo}.git"));
    let bare_s = bare.to_string_lossy().to_string();
    let porcelain =
        crate::git::git_capture(&["--git-dir", &bare_s, "worktree", "list", "--porcelain"], None, false)
            .ok_or_else(|| format!("could not list the worktrees of {repo}"))?;
    let default = crate::cmd::main_sync::default_branch(&bare_s)
        .ok_or_else(|| format!("{repo} has no default branch"))?;
    Ok(plan(
        &porcelain,
        &crate::main_slot::main_worktree(&worktree_dir, repo),
        &bn_core::pool::pool_dir(cfg, repo),
        &default,
    ))
}

/// `reset-to-main` — carry out [`plan_for`], prune, then sync main.
///
/// Needs `--repo`: a fleet-wide force delete is not something a missing argument should mean. The
/// confirmation lives in the picker, before this job's window opens.
pub(crate) fn job_reset_to_main(cfg: &Config, ctx: Option<&Resolved>) -> i32 {
    let Some(ctx) = ctx else {
        eprintln!("reset-to-main: needs --repo (it force-removes that repo's worktrees)");
        return 2;
    };
    let steps = match plan_for(cfg, &ctx.repo) {
        Ok(steps) => steps,
        Err(e) => {
            eprintln!("reset-to-main: {e}");
            return 1;
        }
    };
    let (bare_dir, worktree_dir) = wt::layout(cfg);
    let bare = bare_dir.join(format!("{}.git", ctx.repo));
    let bare_s = bare.to_string_lossy().to_string();
    let mut failed = 0;
    for step in &steps {
        println!("reset-to-main: {}", step.describe());
        let ok = match step {
            // `--force` twice also removes a locked worktree.
            ResetStep::Remove { path, .. } => {
                let p = path.to_string_lossy();
                let ok = wt::run_git(&["--git-dir", &bare_s, "worktree", "remove", "--force", "--force", &p]);
                if ok {
                    wt::rmdir_if_empty_repo(path, &worktree_dir);
                }
                ok
            }
            ResetStep::ReturnSlot { path, .. } => crate::wt_pool::run(
                cfg,
                wt::PoolCmd::Return {
                    target: Some(path.to_string_lossy().to_string()),
                    force: true,
                },
            ),
        };
        if !ok {
            eprintln!("reset-to-main: ✗ {}", step.describe());
            failed += 1;
        }
    }
    let _ = wt::run_git(&["--git-dir", &bare_s, "worktree", "prune"]);
    if steps.is_empty() {
        println!("reset-to-main: no worktrees besides main");
    }
    let synced = crate::cmd::main_sync::sync_repo(&bare, &worktree_dir, &ctx.repo);
    if failed > 0 || !synced {
        eprintln!("reset-to-main: {failed} worktree(s) not removed; main synced: {synced}");
        return 1;
    }
    println!("reset-to-main: done — {} worktree(s) cleared, main is current", steps.len());
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
worktree /b/repo.git
bare

worktree /w/repo/main
HEAD aaa
branch refs/heads/main

worktree /w/repo/_pool/1
HEAD aaa
branch refs/heads/main

worktree /w/repo/_pool/2
HEAD bbb
branch refs/heads/feat/x

worktree /w/repo/_pool/3
HEAD aaa
detached

worktree /w/repo-wt/2026-11-26-idea
HEAD ccc
branch refs/heads/video/idea

worktree /w/repo/review
HEAD ddd
detached
locked

worktree /w/repo/gone
HEAD eee
branch refs/heads/gone
prunable gitdir file points to non-existent location
";

    fn steps() -> Vec<ResetStep> {
        plan(LISTING, Path::new("/w/repo/main"), Path::new("/w/repo/_pool"), "main")
    }

    #[test]
    fn keeps_main_the_main_slot_idle_slots_and_the_bare() {
        let paths: Vec<String> = steps()
            .iter()
            .map(|s| match s {
                ResetStep::Remove { path, .. } | ResetStep::ReturnSlot { path, .. } => {
                    path.display().to_string()
                }
            })
            .collect();
        for kept in ["/b/repo.git", "/w/repo/main", "/w/repo/_pool/1", "/w/repo/_pool/3"] {
            assert!(!paths.contains(&kept.to_string()), "{kept} must stay: {paths:?}");
        }
    }

    #[test]
    fn returns_a_leased_slot_instead_of_deleting_it() {
        assert!(steps().contains(&ResetStep::ReturnSlot {
            path: "/w/repo/_pool/2".into(),
            label: "feat/x".into()
        }));
    }

    #[test]
    fn removes_every_other_worktree_even_outside_the_layout_or_locked() {
        let s = steps();
        assert!(s.contains(&ResetStep::Remove {
            path: "/w/repo-wt/2026-11-26-idea".into(),
            label: "video/idea".into()
        }));
        assert!(s.contains(&ResetStep::Remove {
            path: "/w/repo/review".into(),
            label: "(detached)".into()
        }));
        assert_eq!(s.len(), 3, "{s:?}");
    }

    #[test]
    fn leaves_prunable_registrations_to_git_worktree_prune() {
        assert!(!steps().iter().any(|s| s.describe().contains("/w/repo/gone")));
    }
}
