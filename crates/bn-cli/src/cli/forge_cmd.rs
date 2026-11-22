use crate::*;

#[derive(Subcommand, Debug)]
pub(crate) enum ForgeCmd {
    /// List claimable Forge workspan tasks for a local agent profile.
    ///
    /// Reads `~/.config/wira360/agents/<agent>.json`, validates the OAuth-only MCP profile, then
    /// calls Forge MCP `list_bn_tasks` with `claimable=true`. This is read-only: it never claims,
    /// writes notes, or dispatches work.
    List {
        /// Local Forge agent profile name.
        agent: String,
        /// Show one task id from the claimable set.
        #[arg(long)]
        task: Option<String>,
        /// Maximum rows to print.
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Claim exactly one Forge workspan task for the profile's claimer.
    Claim {
        /// Local Forge agent profile name.
        agent: String,
        /// Forge activity/task id to claim. Required so workspan never guesses among multiple tasks.
        #[arg(long)]
        task: String,
        /// Forge claim lease length in seconds.
        #[arg(long, default_value_t = 900)]
        lease_seconds: u64,
    },
    /// Claim one Forge workspan task, then dispatch it through existing `workspan dispatch`.
    Dispatch {
        /// Local Forge agent profile name.
        agent: String,
        /// Forge activity/task id to claim before dispatch.
        #[arg(long)]
        task: String,
        /// Target workspan repo. Defaults to the current worktree repo when omitted.
        #[arg(long)]
        repo: Option<String>,
        /// Harness passed to `workspan dispatch --agent`.
        #[arg(long = "agent", default_value = "copilot")]
        agent_harness: String,
        /// Base branch passed to `workspan dispatch --base`.
        #[arg(long)]
        base: Option<String>,
        /// Workflow skill passed to `workspan dispatch --skill`.
        #[arg(long, default_value = "forge-task")]
        skill: String,
        /// Readable task name for the local workspan agent.
        #[arg(long)]
        name: Option<String>,
        /// Gate the dispatched agent on `workspan verify`.
        #[arg(long)]
        verify: bool,
        /// Forge claim lease length in seconds.
        #[arg(long, default_value_t = 900)]
        lease_seconds: u64,
    },
    /// Poll Forge once: reap abandoned claims, then dispatch newly assigned work.
    ///
    /// The unattended half of `workspan forge`. Every in-flight task this agent still holds is checked
    /// against the live fleet: a claim whose worker is gone is released back to Forge rather than
    /// held forever, and a claim whose worker is alive gets its lease extended. Remaining capacity
    /// is then filled from the claimable queue. Designed to run from cron every ~10 minutes.
    Tick {
        /// Local Forge agent profile name.
        agent: String,
        /// Target workspan repo. Defaults to the current worktree repo when omitted.
        #[arg(long)]
        repo: Option<String>,
        /// Harness passed to `workspan dispatch --agent`.
        #[arg(long = "agent", default_value = "copilot")]
        agent_harness: String,
        /// Base branch passed to `workspan dispatch --base`.
        #[arg(long)]
        base: Option<String>,
        /// Workflow skill passed to `workspan dispatch --skill`.
        #[arg(long, default_value = "forge-task")]
        skill: String,
        /// Maximum tasks this agent may have in flight at once.
        #[arg(long, default_value_t = 2)]
        max_active: usize,
        /// Report what the tick would do without claiming, dispatching, or releasing anything.
        #[arg(long)]
        dry_run: bool,
        /// Forge claim lease length in seconds.
        #[arg(long, default_value_t = 900)]
        lease_seconds: u64,
    },
}
