//! The machine's repo inventory: every bare clone under the worktree layout, with the facts a
//! caller needs to pick one. Shared by `workspan repos list` and the `workspan_repos` MCP tool so
//! both report the same thing. Bare clones only — a flat clone has no `<name>.git` here.
use crate::resolver::Config;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One cloned repo.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepoInfo {
    /// Directory stem of the bare clone, in its real case.
    pub repo: String,
    pub git_dir: PathBuf,
    /// The bare clone's `HEAD` branch; empty when git cannot say.
    pub default_branch: String,
    /// `remote.origin.url`, when the clone has one.
    pub remote: Option<String>,
    /// The worktree on the default branch, when it exists at its conventional path.
    pub main_worktree: Option<PathBuf>,
}

/// The directory the bare clones live in, honoring `$WT_ROOT`.
pub fn bare_dir(cfg: &Config) -> PathBuf {
    crate::pool::bare_root(cfg)
}

/// Every bare clone, in name order. Empty when the bare dir is missing or unreadable.
pub fn inventory(cfg: &Config) -> Vec<RepoInfo> {
    let worktree_root = crate::pool::worktree_root(cfg);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(bare_dir(cfg))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|x| x == "git"))
        .collect();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|git_dir| {
            let repo = git_dir.file_stem()?.to_string_lossy().to_string();
            let default_branch =
                git_in(&git_dir, &["symbolic-ref", "--short", "HEAD"]).unwrap_or_default();
            let remote = git_in(&git_dir, &["config", "--get", "remote.origin.url"])
                .filter(|s| !s.is_empty())
                .map(|url| redact_credentials(&url));
            let main_worktree = (!default_branch.is_empty())
                .then(|| {
                    worktree_root
                        .join(&repo)
                        .join(default_branch.replace('/', "-"))
                })
                .filter(|p| p.is_dir());
            Some(RepoInfo {
                repo,
                git_dir,
                default_branch,
                remote,
                main_worktree,
            })
        })
        .collect()
}

/// A remote URL with any embedded credential removed, so a token stored in the clone's config
/// (`https://user:token@host/...`) never reaches an agent or a log. HTTP(S) drops the whole
/// userinfo; other schemes keep the login name (`ssh://git@host`) and drop only a password. The
/// scp form `git@host:path` has no password part and passes through.
fn redact_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let Some((userinfo, host)) = authority.rsplit_once('@') else {
        return url.to_string();
    };
    let user = userinfo.split(':').next().unwrap_or_default();
    if scheme.starts_with("http") || user.is_empty() {
        format!("{scheme}://{host}{path}")
    } else {
        format!("{scheme}://{user}@{host}{path}")
    }
}

/// `git -C <dir> <args>` → trimmed stdout, or `None` when git failed.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{config_with_roots, env_lock, EnvGuard};

    fn bare(root: &Path, name: &str) -> PathBuf {
        let dir = root.join("bare").join(format!("{name}.git"));
        let ok = Command::new("git")
            .args(["init", "--quiet", "--bare", "--initial-branch=main"])
            .arg(&dir)
            .status()
            .expect("git init")
            .success();
        assert!(ok);
        dir
    }

    fn cfg(root: &Path) -> Config {
        config_with_roots(root, root, root.join("worktree"), "testhost")
    }

    #[test]
    fn lists_bare_clones_in_name_order_with_their_facts() {
        let _lock = env_lock();
        let _env = EnvGuard::remove("WT_ROOT");
        let root = tempfile::tempdir().unwrap();
        bare(root.path(), "zeta");
        let big = bare(root.path(), "BigMono");
        git_in(
            &big,
            &[
                "config",
                "remote.origin.url",
                "https://example.com/o/BigMono.git",
            ],
        )
        .expect("set remote");
        let main = root.path().join("worktree/BigMono/main");
        std::fs::create_dir_all(&main).unwrap();
        std::fs::create_dir_all(root.path().join("bare/not-a-repo")).unwrap();

        let repos = inventory(&cfg(root.path()));

        assert_eq!(
            repos.iter().map(|r| r.repo.as_str()).collect::<Vec<_>>(),
            ["BigMono", "zeta"]
        );
        assert_eq!(repos[0].default_branch, "main");
        assert_eq!(
            repos[0].remote.as_deref(),
            Some("https://example.com/o/BigMono.git")
        );
        assert_eq!(repos[0].main_worktree.as_deref(), Some(main.as_path()));
        assert_eq!(repos[1].remote, None);
        assert_eq!(repos[1].main_worktree, None);
    }

    #[test]
    fn credentials_never_leave_the_remote_url() {
        for (url, want) in [
            (
                "https://me:ghp_secret@github.com/o/r.git",
                "https://github.com/o/r.git",
            ),
            (
                "https://ghp_secret@github.com/o/r.git",
                "https://github.com/o/r.git",
            ),
            (
                "https://org@dev.azure.com/org/p/_git/r",
                "https://dev.azure.com/org/p/_git/r",
            ),
            (
                "ssh://git:pw@host.example/o/r.git",
                "ssh://git@host.example/o/r.git",
            ),
            (
                "ssh://git@host.example/o/r.git",
                "ssh://git@host.example/o/r.git",
            ),
            ("git@github.com:o/r.git", "git@github.com:o/r.git"),
            (
                "https://github.com/o/r@v1.git",
                "https://github.com/o/r@v1.git",
            ),
        ] {
            assert_eq!(redact_credentials(url), want, "{url}");
        }
    }

    #[test]
    fn a_missing_bare_dir_is_an_empty_inventory() {
        let _lock = env_lock();
        let _env = EnvGuard::remove("WT_ROOT");
        let root = tempfile::tempdir().unwrap();
        assert!(inventory(&cfg(root.path())).is_empty());
    }
}
