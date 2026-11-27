//! The **opt-in** orchestrator MCP server (`workspan-mcp --orchestrate`) — the cross-worktree surface the
//! "friend" drives. It is deliberately SEPARATE from the scoped [`crate::server::BnServer`]: that
//! server is locked to one worktree so an agent can't touch unrelated repos, and this one is its
//! escape hatch, registered only in the orchestrator's own environment.
//!
//! Every tool composes the real `workspan` CLI (resolved binary path, never a bare PATH guess) rather than
//! re-implementing dispatch/fleet/attention logic — so there is one source of truth (the CLI) and
//! bn-core stays tmux-free. The tools are thin: resolve `workspan`, run a subcommand with `--json`, hand
//! the envelope back.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct BnOrchestratorServer;

impl Default for BnOrchestratorServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmptyArgs {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SpinUpArgs {
    /// Target repo by name (a bare repo under the bare root). Works from anywhere.
    repo: String,
    /// The goal / item to work on — becomes the new branch note's Goal and the agent's seed. May be
    /// free text, or an issue/PR URL (its title is resolved).
    goal: String,
    /// Branch to create (default: derived from the goal as `dispatch/<slug>`). For attention
    /// fanout, pass a unique branch (e.g. `dispatch/<slug>-<attn-id>`) so items can't collide.
    #[serde(default)]
    branch: Option<String>,
    /// Base branch to fork from (default: main).
    #[serde(default)]
    base: Option<String>,
    /// Agent to launch in the worktree window: `copilot` (default) or `claude`. Empty/absent →
    /// copilot. Pass a literal `none` to open a plain shell window with no agent.
    #[serde(default)]
    agent: Option<String>,
    /// Workflow skill to seed the agent with (see the workspan skill prompts): its rendered body becomes
    /// the agent's process, ending with a `workspan reflect "skill:<name> — …"` line. Defaults to
    /// `ship-solo`; pass `debug` for a repro-first bugfix.
    #[serde(default)]
    skill: Option<String>,
    /// Gate "done" on verification: the ship agent must run `workspan verify` (the repo's build/test/lint gate)
    /// and only report done once it passes. Default false.
    #[serde(default)]
    verify: bool,
    /// Trusted-repo autonomy: launch the agent without approval prompts so it runs clean instead of
    /// wedging on first-run trust/MCP prompts. Only for repos the user trusts. Default false.
    #[serde(default)]
    yolo: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateWorktreeArgs {
    /// Target repo by name, as `workspan_repos` / `workspan repos list` spells it.
    repo: String,
    /// Branch to check out. An existing local or remote branch is used as-is; otherwise a new
    /// branch is cut from `base`.
    branch: String,
    /// Base branch a NEW branch forks from (default: the repo's default branch).
    #[serde(default)]
    base: Option<String>,
    /// Cut a named worktree at `<repo>/<branch>` instead of leasing a warm pool slot. Slower (no
    /// cached deps) and never reclaimed automatically; use only when the pool is unavailable.
    #[serde(default)]
    cold: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CloneRepoArgs {
    /// Git URL: `https://…`, `ssh://…` or `git@host:owner/repo.git`. No other form is accepted.
    url: String,
    /// Repo name to clone as (default: the URL's basename without `.git`).
    #[serde(default)]
    name: Option<String>,
}

/// A value that becomes one path component or ref on a git command line: non-empty, not a flag,
/// and free of the characters that would let it mean something else.
fn plain_name(what: &str, value: &str) -> Result<(), ErrorData> {
    let bad = value.is_empty()
        || value.starts_with('-')
        || value.contains("..")
        || value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\');
    if bad {
        return Err(ErrorData::invalid_params(
            format!("{what} '{value}' is not a plain name"),
            None,
        ));
    }
    Ok(())
}

/// Only network transports a person would paste: `https://`, `ssh://`, or scp-style
/// `user@host:path`. Everything else is refused before git sees it — `ext::` runs an arbitrary
/// command, `file://` and bare paths read the local disk, and a leading `-` is an option.
fn check_clone_url(url: &str) -> Result<(), ErrorData> {
    let refuse = |why: &str| {
        Err(ErrorData::invalid_params(
            format!("clone_repo: {why}; pass an https://, ssh:// or git@host:owner/repo.git URL"),
            None,
        ))
    };
    if url.is_empty() || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return refuse("the URL is empty or contains whitespace");
    }
    if url.starts_with('-') {
        return refuse("the URL looks like an option");
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("ssh://") {
        return Ok(());
    }
    let scp = url.split_once(':').is_some_and(|(login, path)| {
        let host_ok = |h: &str| {
            !h.is_empty()
                && h.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
        };
        !path.is_empty()
            && !path.starts_with(':')
            && !path.starts_with('/')
            && login.split_once('@').is_some_and(|(user, host)| {
                !user.is_empty()
                    && user
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
                    && host_ok(host)
            })
    });
    if scp {
        Ok(())
    } else {
        refuse("that transport is not allowed")
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SendArgs {
    /// The agent's tmux window, as `repo:branch` (the `target` from `list_agents`).
    target: String,
    /// The message to send.
    msg: String,
    /// Fire-and-forget for a LONG task (implement + PR): run the turn detached and return now; the
    /// agent reports back with `workspan agent done`. Use this for anything that will take minutes — a
    /// blocking send would time out. Default false (waits for the reply, good for quick questions).
    #[serde(default)]
    detach: bool,
}

#[tool_router]
impl BnOrchestratorServer {
    pub fn new() -> Self {
        Self
    }

    /// Run `workspan --json <args...>` and return the parsed JSON envelope. Errors carry the CLI's stderr.
    fn run_bn(&self, args: &[&str]) -> Result<Value, ErrorData> {
        self.run_bn_env(args, &[])
    }

    /// Like [`Self::run_bn`], but sets extra env on the child. `spin_up_work` uses it to declare
    /// `BN_WORKER_REPO=<target>` so the CLI's cross-repo-session guard treats a spin-up as the
    /// target repo's own session (this server runs from its own worktree, so it would otherwise
    /// look like a stray cross-repo hop and be blocked).
    fn run_bn_env(&self, args: &[&str], envs: &[(&str, &str)]) -> Result<Value, ErrorData> {
        let out = bn_core::bn_subprocess::run_with_env(args, None, envs)
            .map_err(|e| ErrorData::internal_error(format!("failed to run {e}"), None))?;
        if !out.success {
            let stderr = out.stderr.trim();
            let stdout = out.stdout.trim();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(ErrorData::internal_error(
                format!("workspan {}: {detail}", args.join(" ")),
                None,
            ));
        }
        // The `--json` envelope is the LAST line, but some subcommands (e.g. dispatch's tmux
        // window-open) print a human line to stdout first. Take the last JSON-parseable line so we
        // tolerate that leading noise instead of choking on it.
        out.json.ok_or_else(|| {
            ErrorData::internal_error(
                format!("workspan {} returned no JSON envelope", args.join(" ")),
                None,
            )
        })
    }

    #[tool(
        name = "list_attention",
        description = "The open attention list across all repos, each item annotated with a triage class: \"review\" (a finished agent's completion — source `agent`; review/merge its result, don't re-dispatch), \"auto\" (fully routable to a worktree — has both repo and branch — safe to spin up unattended), or \"confirm\" (no repo/branch, or a judgment call — ask first). Use this to decide what to spin_up_work autonomously, what to surface, and which agents just finished."
    )]
    async fn list_attention(
        &self,
        Parameters(_a): Parameters<EmptyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // Two lanes since #242: `attn list` is the actionable open set (completions filtered
        // out); the finished-agent completions this tool classifies as "review" live in the
        // feed's review inbox. Read both so a done agent still surfaces here.
        let env = self.run_bn(&["attn", "list"])?;
        let mut items = env
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();
        let feed = self.run_bn(&["feed"])?;
        items.extend(
            feed.get("data")
                .and_then(|d| d.get("completed"))
                .and_then(|i| i.as_array())
                .cloned()
                .unwrap_or_default(),
        );
        let classify = |it: &Value| -> &'static str {
            let has = |k: &str| {
                it.get(k)
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty())
            };
            let source = it.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("");
            // A finished agent's completion push (source `agent`, "done: …") is NOT new work to
            // spin up — it's a result to review/merge. Classify it apart so the buddy never
            // re-dispatches a done item.
            if source == "agent" || title.starts_with("done:") {
                "review"
            } else if has("repo") && has("branch") {
                // Auto only when the item routes to a specific worktree — the unambiguous case
                // (e.g. an auto-review PR). Anything without repo+branch needs a human to place it.
                "auto"
            } else {
                "confirm"
            }
        };
        let annotated: Vec<Value> = items
            .iter()
            .map(|it| {
                let mut o = it.clone();
                if let Some(map) = o.as_object_mut() {
                    map.insert("triage".into(), json!(classify(it)));
                }
                o
            })
            .collect();
        let count = |cls: &str| {
            annotated
                .iter()
                .filter(|it| it.get("triage").and_then(|v| v.as_str()) == Some(cls))
                .count()
        };
        Ok(CallToolResult::structured(json!({
            "items": annotated,
            "auto": count("auto"),
            "confirm": count("confirm"),
            "review": count("review"),
        })))
    }

    #[tool(
        name = "list_agents",
        description = "The joined live agent fleet across all worktrees and both harnesses (Copilot + Claude): one row per repo:branch with its tmux state (working/waiting/done), health (active/idle/stuck), and whether a window exists. This is \"what are all my agents doing right now\"."
    )]
    async fn list_agents(
        &self,
        Parameters(_a): Parameters<EmptyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let env = self.run_bn(&["agent", "live"])?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(json!({})),
        ))
    }

    #[tool(
        name = "create_worktree",
        description = "Get a working directory for a branch in any cloned repo, with no agent, task or tmux window: leases a warm pool slot (deps already cached), checks the branch out there (cutting it from `base` if it is new), and returns `worktree` — the path to work in. Idempotent: asking again for the same branch returns the same directory with `created: false`, and a branch already checked out elsewhere returns that directory. The default branch returns the repo's main worktree. If the pool is full the call fails with what to do; pass `cold: true` only then. When finished, hand a pooled slot back with `workspan wt pool return <worktree>`. Find repo names with workspan_repos or `workspan repos list`."
    )]
    async fn create_worktree(
        &self,
        Parameters(a): Parameters<CreateWorktreeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        plain_name("repo", &a.repo)?;
        plain_name("branch", &a.branch)?;
        let mut args: Vec<String> = vec![
            "wt".into(),
            "pool".into(),
            "get".into(),
            format!("--repo={}", a.repo),
        ];
        if let Some(b) = a.base.as_deref().filter(|s| !s.is_empty()) {
            plain_name("base", b)?;
            args.push(format!("--base={b}"));
        }
        if a.cold {
            args.push("--cold".into());
        }
        args.push("--".into());
        args.push(a.branch.clone());
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let env = self.run_bn_env(&argv, &[("BN_WORKER_REPO", &a.repo)])?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(env),
        ))
    }

    #[tool(
        name = "clone_repo",
        description = "Clone a git repo onto this machine in workspan's bare + worktree layout, so it can pool worktrees. Returns the repo name, its git dir and the default branch's worktree. Idempotent: a repo that is already cloned is reported with `created: false`. Accepts only https://, ssh:// and git@host:owner/repo.git URLs. Large repos take minutes. Follow with create_worktree to get a branch to work in."
    )]
    async fn clone_repo(
        &self,
        Parameters(a): Parameters<CloneRepoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        check_clone_url(&a.url)?;
        let mut args = vec!["wt", "clone", "--", a.url.as_str()];
        if let Some(name) = a.name.as_deref().filter(|s| !s.is_empty()) {
            plain_name("name", name)?;
            if name.contains('/') {
                return Err(ErrorData::invalid_params("name must not contain '/'", None));
            }
            args.push(name);
        }
        let env = self.run_bn(&args)?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(env),
        ))
    }

    #[tool(
        name = "spin_up_work",
        description = "Spin up isolated work on any repo from anywhere: create a worktree off the base branch, seed its branch note with the goal, open a window in the repo's tmux session, and launch an agent (copilot default; claude also). Each call is fully isolated — its own worktree, tmux window, and note. Re-calling the same branch is idempotent (won't clobber a live note)."
    )]
    async fn spin_up_work(
        &self,
        Parameters(a): Parameters<SpinUpArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // Build argv defensively against flag smuggling: option values use the attached `--k=v`
        // form (so a value starting with `-` is taken literally, never parsed as a flag), and the
        // free-text `goal` is placed after a `--` separator so it can never be read as an option —
        // even if the caller passes goal = "--no-tmux".
        let mut args: Vec<String> = vec!["dispatch".into(), format!("--repo={}", a.repo)];
        if let Some(b) = a.branch.as_deref().filter(|s| !s.is_empty()) {
            args.push(format!("--branch={b}"));
        }
        if let Some(b) = a.base.as_deref().filter(|s| !s.is_empty()) {
            args.push(format!("--base={b}"));
        }
        match a.agent.as_deref().map(str::trim) {
            Some("none") => { /* plain shell window: pass no --agent */ }
            Some(k) if !k.is_empty() => args.push(format!("--agent={k}")),
            _ => args.push("--agent=copilot".into()), // default
        }
        if let Some(s) = a.skill.as_deref().filter(|s| !s.is_empty()) {
            args.push(format!("--skill={s}"));
        }
        if a.verify {
            args.push("--verify".into());
        }
        if a.yolo {
            args.push("--yolo".into());
        }
        args.push("--".into());
        args.push(a.goal.clone());
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        // Declare the target as this dispatch's session so the CLI's cross-repo-session guard
        // treats it as sandbox's own session (spin_up_work is the sanctioned cross-repo path).
        let env = self.run_bn_env(&argv, &[("BN_WORKER_REPO", &a.repo)])?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(env),
        ))
    }

    #[tool(
        name = "send_to_agent",
        description = "Steer a running agent (repo:branch from list_agents) and get its reply — reliable, works even against a live session. Validated against the live fleet first. For a LONG task (implement + open a PR), set detach:true so it runs in the background and reports back via `workspan agent done` — a blocking send would time out. For a quick question, leave detach false to get the reply."
    )]
    async fn send_to_agent(
        &self,
        Parameters(a): Parameters<SendArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // Validate the target against the live fleet before sending (never send blind).
        let fleet = self.run_bn(&["agent", "live"])?;
        let live = fleet
            .get("data")
            .and_then(|d| d.get("agents"))
            .and_then(|a| a.as_array())
            .map(|agents| {
                agents.iter().any(|ag| {
                    ag.get("target").and_then(|t| t.as_str()) == Some(a.target.as_str())
                        && ag.get("has_window").and_then(|w| w.as_bool()) == Some(true)
                })
            })
            .unwrap_or(false);
        if !live {
            return Err(ErrorData::invalid_params(
                format!(
                    "no live agent window for target '{}' — check list_agents",
                    a.target
                ),
                None,
            ));
        }
        // `--` before the positionals so a target/msg starting with `-` can't smuggle a flag.
        let mut args: Vec<&str> = vec!["agent", "send"];
        if a.detach {
            args.push("--detach");
        }
        args.extend(["--", &a.target, &a.msg]);
        let env = self.run_bn(&args)?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(env),
        ))
    }

    #[tool(
        name = "plan_day",
        description = "The ranked cross-repo action queue (workspan next): failing crons, attention items, PRs awaiting your review, your stale PRs, today's goals, and blockers — most-pressing first. The \"what's up / what to do next today\" read the buddy leads with."
    )]
    async fn plan_day(
        &self,
        Parameters(_a): Parameters<EmptyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let env = self.run_bn(&["next"])?;
        Ok(CallToolResult::structured(
            env.get("data").cloned().unwrap_or(json!({})),
        ))
    }
}

#[tool_handler]
impl ServerHandler for BnOrchestratorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "bn-orchestrator",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Workspan orchestrator — the opt-in CROSS-WORKTREE surface (separate from the scoped \
                 per-worktree workspan server). Manage the whole desk from anywhere: plan_day for the \
                 ranked next-action queue; list_attention for the attention list (each item tagged \
                 auto vs confirm — spin up the auto ones unattended, ask about the confirm ones); \
                 spin_up_work to create an isolated worktree+tmux window+seeded agent on any repo; \
                 create_worktree for just a working directory on a branch (no agent, no task), and \
                 clone_repo to bring a new repo onto the machine; \
                 list_agents for the live fleet (both harnesses) and send_to_agent to steer one. \
                 Every spun-up agent is fully isolated and keeps its own workspan note."
                    .to_string(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_urls_are_limited_to_network_transports() {
        for ok in [
            "https://github.com/o/r.git",
            "HTTPS://dev.azure.com/org/p/_git/r",
            "ssh://git@github.com/o/r.git",
            "git@github.com:o/r.git",
        ] {
            assert!(check_clone_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "--upload-pack=touch /tmp/x",
            "-u",
            "ext::sh -c 'id'",
            "ext::sh",
            "file:///etc",
            "/tmp/local/repo",
            "../repo",
            "http://plain.example/r.git",
            "git://host.example/r.git",
            "fd::3",
            "git@host:/abs/path",
            "-oProxyCommand=x@host:r",
            "https://ok.example/r.git --upload-pack=x",
        ] {
            assert!(check_clone_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_that_could_be_flags_or_paths_are_refused() {
        for ok in ["workspan", "BigMono", "feat/x", "dispatch/a-b_c.1"] {
            assert!(plain_name("branch", ok).is_ok(), "{ok}");
        }
        for bad in ["", "-b", "--cold", "a b", "../up", "a\\b", "x\ny"] {
            assert!(plain_name("branch", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn worktree_and_clone_args_reject_unknown_fields() {
        let args = json!({"repo": "r", "branch": "b"});
        let parsed = serde_json::from_value::<CreateWorktreeArgs>(args).unwrap();
        assert!(!parsed.cold && parsed.base.is_none());
        let args = json!({"repo": "r", "branch": "b", "agent": "claude"});
        assert!(serde_json::from_value::<CreateWorktreeArgs>(args).is_err());
        let args = json!({"url": "https://x/r.git", "depth": 1});
        assert!(serde_json::from_value::<CloneRepoArgs>(args).is_err());
    }

    #[test]
    fn spin_up_work_rejects_the_removed_scout_option() {
        let args = json!({"repo": "repo", "goal": "work", "scout": true});
        assert!(serde_json::from_value::<SpinUpArgs>(args).is_err());
        let args = json!({"repo": "repo", "goal": "work", "skill": "debug", "verify": true});
        let parsed = serde_json::from_value::<SpinUpArgs>(args).unwrap();
        assert_eq!(parsed.skill.as_deref(), Some("debug"));
        assert!(parsed.verify);
    }
}
