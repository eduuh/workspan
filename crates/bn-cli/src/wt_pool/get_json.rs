//! `workspan --json wt pool get` — the machine form of leasing a worktree, for callers that cannot
//! answer a prompt (the orchestrator MCP's `create_worktree`). It never opens fzf or tmux: repo and
//! branch are required, and the answer is one JSON object naming the directory to work in.
//!
//! A full pool is an error, not a silent cold checkout — nothing reclaims a cold worktree, so one
//! is only cut when the caller asks for it with `--cold`.
use super::*;
use serde_json::{json, Value};

/// Lease (or find) a worktree for `branch` in `repo`. `Err` is an envelope `(code, message)`.
pub(crate) fn get_worktree(
    cfg: &Config,
    repo: &str,
    branch: &str,
    base: Option<&str>,
    holder: Option<String>,
    cold: bool,
) -> Result<Value, (&'static str, String)> {
    let Some(ctx) = resolve_dispatch_pool(cfg, repo) else {
        return Err((
            "missing_repo",
            format!("no bare clone named '{repo}'; `workspan repos list` shows what is cloned"),
        ));
    };
    let bare_s = ctx.bare.to_string_lossy().to_string();
    let done = |worktree: &Path, created: bool, pooled: bool| {
        json!({
            "repo": ctx.repo,
            "branch": branch,
            "base": base,
            "worktree": worktree.to_string_lossy(),
            "created": created,
            "pooled": pooled,
        })
    };

    // Best effort, and never a credential prompt: an offline machine still gets its worktree.
    let _ = crate::git::git_command(&["--git-dir", &bare_s, "fetch", "--quiet", "origin"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // git checks a branch out once. If it already lives outside the pool (a named worktree, or the
    // default branch's own), that directory is the answer; a slot holding it is the lease's to find.
    if let Some(row) = wt::worktree_rows(&ctx.bare)
        .into_iter()
        .find(|r| r.branch.as_deref() == Some(branch) && slot_parts(&r.path).is_none())
    {
        return Ok(done(&row.path, false, false));
    }

    let default = wt::resolve_default_branch(&bare_s);
    if cold || default.as_deref() == Some(branch) {
        let path = ctx
            .worktree_dir
            .join(&ctx.repo)
            .join(wt::sanitize_branch(branch));
        let base = base.or(default.as_deref()).unwrap_or("main");
        let created = crate::cmd::dispatch::create_worktree(&ctx.bare, &path, branch, base)
            .map_err(|e| {
                (
                    "worktree_failed",
                    format!("could not create {}: {e}", path.display()),
                )
            })?;
        return Ok(done(&path, created, false));
    }

    let Some(leased) = acquire_slot(&ctx, branch, base, holder, SlotRole::Worker, false) else {
        return Err((
            "pool_unavailable",
            format!(
                "no pool slot could be leased for {}:{branch} (pool full, or the branch could not be \
                 switched in). Check `workspan wt pool status --repo {}`, hand a slot back with \
                 `workspan wt pool return`, or pass --cold for a named worktree.",
                ctx.repo, ctx.repo
            ),
        ));
    };
    // Pinned like any direct `pool get`: nobody may be sitting in the slot yet, and an unpinned
    // lease with no live process reads as abandoned.
    lease_store::pin(&ctx.pool_dir, &leased.slot);
    crate::cmd::trust::apply_folder_trust(&leased.slot.to_string_lossy(), "copilot");
    Ok(done(&leased.slot, leased.fresh, true))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pool_get(
    cfg: &Config,
    branch: Option<String>,
    repo: Option<String>,
    holder: Option<String>,
    base: Option<String>,
    cold: bool,
    path_only: bool,
    no_tmux: bool,
) -> bool {
    if cold {
        ceprintln!(
            "{RED}--cold needs --json; `workspan wt add` cuts a named worktree by hand.{NC}"
        );
        return false;
    }
    let Some(ctx) = resolve_pool(cfg, repo) else {
        return false;
    };
    let Some(branch) = branch else {
        ceprintln!("{RED}Usage: wt pool get <branch>{NC}");
        return false;
    };
    let Some(leased) = acquire_slot(
        &ctx,
        &branch,
        base.as_deref(),
        holder,
        SlotRole::Worker,
        false,
    ) else {
        return false;
    };
    lease_store::pin(&ctx.pool_dir, &leased.slot);
    finish_get(&ctx, &leased.slot, &branch, path_only, no_tmux)
}

#[cfg(test)]
mod tests {
    use super::super::git_integration_tests::build_repo_into;
    use super::*;
    use bn_core::test_support::{config, env_lock, EnvGuard};

    /// A `<root>/bare/bn.git` clone with `main` checked out at `<root>/worktree/bn/main`.
    fn layout(root: &Path) -> Config {
        let bare = root.join("bare").join("bn.git");
        build_repo_into(&root.join("src"), &bare);
        let main = root.join("worktree").join("bn").join("main");
        assert!(wt::run_git(&[
            "--git-dir",
            &bare.to_string_lossy(),
            "worktree",
            "add",
            "--quiet",
            &main.to_string_lossy(),
            "main",
        ]));
        TEST_OCCUPANTS.with(|c| *c.borrow_mut() = Some(BTreeMap::new()));
        config(root)
    }

    #[test]
    fn leases_a_pool_slot_and_hands_the_same_one_back() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let _wt_root = EnvGuard::set("WT_ROOT", tmp.path());
        let cfg = layout(tmp.path());

        let first = get_worktree(&cfg, "BN", "feat", None, None, false).expect("lease");
        assert_eq!(first["repo"], "bn", "the clone's own spelling");
        assert_eq!(
            (&first["created"], &first["pooled"]),
            (&json!(true), &json!(true))
        );
        let slot = PathBuf::from(first["worktree"].as_str().unwrap());
        assert!(slot_parts(&slot).is_some(), "{slot:?} is a pool slot");
        assert!(read_leases(slot.parent().unwrap())
            .slots
            .values()
            .any(|l| l.pinned));

        let again = get_worktree(&cfg, "bn", "feat", None, None, false).expect("re-lease");
        assert_eq!(again["worktree"], first["worktree"]);
        assert_eq!(again["created"], false);
    }

    #[test]
    fn the_default_branch_is_its_existing_worktree_not_a_slot() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let _wt_root = EnvGuard::set("WT_ROOT", tmp.path());
        let cfg = layout(tmp.path());

        let got = get_worktree(&cfg, "bn", "main", None, None, false).expect("main");
        assert!(got["worktree"]
            .as_str()
            .unwrap()
            .ends_with("worktree/bn/main"));
        assert_eq!(
            (&got["created"], &got["pooled"]),
            (&json!(false), &json!(false))
        );
    }

    #[test]
    fn cold_cuts_a_named_worktree_only_when_asked() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let _wt_root = EnvGuard::set("WT_ROOT", tmp.path());
        let cfg = layout(tmp.path());

        let got = get_worktree(&cfg, "bn", "topic/new", Some("main"), None, true).expect("cold");
        let path = tmp.path().join("worktree/bn/topic-new");
        assert_eq!(got["worktree"], json!(path.to_string_lossy()));
        assert_eq!(
            (&got["created"], &got["pooled"]),
            (&json!(true), &json!(false))
        );
        assert!(path.join("a.txt").is_file());
        // Asking again, pooled or not, finds the directory that already holds the branch.
        let again = get_worktree(&cfg, "bn", "topic/new", None, None, false).expect("found");
        assert_eq!(again["worktree"], got["worktree"]);
        assert_eq!(again["created"], false);
    }

    #[test]
    fn an_unknown_repo_is_a_named_error() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let _wt_root = EnvGuard::set("WT_ROOT", tmp.path());
        let cfg = layout(tmp.path());

        let err = get_worktree(&cfg, "nope", "feat", None, None, false).unwrap_err();
        assert_eq!(err.0, "missing_repo");
    }
}
