<<<<<<< HEAD
//! Worktree-scoped reference and command tools, person-scoped links, and workflow prompts.
//! Session history belongs to Entire; this server does not read or write branch notes.
=======
//! The MCP server: commands, references and durable links, plus workflow prompts from every skill
//! tier. Deliberately narrower than the CLI: cross-branch, whole-person, and explicit repo/branch
//! targeting stay in `bn` so an MCP-connected agent cannot discover or act on unrelated sessions.
//!
//! The *tool* surface is what enforces that narrowness. Prompts are inert text — serving one only
//! puts a workflow's instructions in front of the agent, and every action it then takes still goes
//! through the same scoped tools. So the prompt list is not a security boundary and is no longer
//! filtered: an allowlist here only hid skills from discovery while `bn skills` listed them all.
>>>>>>> retire-notes-runtime

use std::path::PathBuf;

use bn_core::links;
use bn_core::resolver::{self, Config, Resolved};
<<<<<<< HEAD
use bn_core::skills::{load_skill_set, Skill};
=======
>>>>>>> retire-notes-runtime
use bn_core::util::today;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult,
    PaginatedRequestParams, PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[cfg(test)]
mod tests;

pub struct BnServer {
    cfg: Config,
    /// Fixed launch directory. Resolve its current branch per call, never from request data.
    worktree: PathBuf,
    /// Re-scanned on each prompt list/get so newly added skill files appear without restarting.
    skill_dirs: Vec<PathBuf>,
<<<<<<< HEAD
    /// Serializes read-modify-write operations on reference docs and links.
=======
    /// Serializes reference and link read-modify-write operations within this server.
>>>>>>> retire-notes-runtime
    write_lock: std::sync::Mutex<()>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LinksSearchArgs {
    /// Words to find in a link's name, tags or URL (case-insensitive). Omit to list by rank.
    #[serde(default)]
    query: Option<String>,
    /// Only links carrying this tag.
    #[serde(default)]
    tag: Option<String>,
    /// Most links to return (default 20).
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LinksAddArgs {
    /// The http(s) URL to store. Anything else is rejected.
    url: String,
    /// Broad category this durable link fills. Must be in the configured link vocabulary.
    category: String,
    /// What to call it in the picker. Omit to use the host plus the last path segment.
    #[serde(default)]
    name: Option<String>,
    /// Optional topic tags beyond the required category.
    #[serde(default)]
    tags: Vec<String>,
    /// Repository this link belongs to. Omit to use this worktree's repo; pass "" for none.
    #[serde(default)]
    repo: Option<String>,
    /// Hide the link after this long: "7d", "2w", or a day count.
    #[serde(default)]
    ttl: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RefArgs {
    /// Reference to read in full. Omit to get the index first.
    #[serde(default)]
    name: Option<String>,
    /// Which store: `repo`, `global`, or `all` (default).
    #[serde(default)]
    scope: Option<String>,
    /// Read only this section of the named reference. `name#section` works too.
    #[serde(default)]
    section: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RefAppendArgs {
    /// Reference document to write to in this repo's store. Created if it does not exist.
    name: String,
    /// One durable fact, including its source and verification date.
    text: String,
    /// Section heading to append under: `facts` (default), `gotchas`, or any other.
    #[serde(default)]
    section: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CmdArgs {
    /// The command to run. Omit to list commands with their description and usage.
    #[serde(default)]
    name: Option<String>,
    /// Arguments passed to the script as `$1`, `$2`, ...
    #[serde(default)]
    args: Vec<String>,
    /// Kill the command after this many seconds (default 120, max 600).
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[tool_router]
impl BnServer {
    pub fn new(cfg: Config, worktree: PathBuf, skill_dirs: Vec<PathBuf>) -> Self {
        Self {
            cfg,
            worktree,
            skill_dirs,
            write_lock: std::sync::Mutex::new(()),
        }
    }

<<<<<<< HEAD
=======
    /// The retained command, reference and link tool surface.
    fn all_tools() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }

    /// Every effective skill, freshly merged from `skill_dirs` (higher tiers override lower ones
    /// by name, so each prompt appears once). Re-read per call, so a newly authored skill shows up
    /// without restarting the server.
>>>>>>> retire-notes-runtime
    fn skills(&self) -> Result<Vec<Skill>, ErrorData> {
        load_skill_set(&self.skill_dirs)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }

    fn resolve(&self) -> Result<Resolved, ErrorData> {
        resolver::resolve(&self.cfg, &self.worktree).map_err(|_| {
            ErrorData::invalid_params(
                "no worktree scope -- run `bn mcp serve` from inside a git worktree",
                None,
            )
        })
    }

    #[tool(
        name = "bn_links_search",
        description = "Search the person-scoped link store before asking the user for a URL they have likely saved. Matches words against each link's name, tags and URL; this repo's links rank first. Read-only, and like bn_links_add it works even when no branch resolves."
    )]
    async fn bn_links_search(
        &self,
        Parameters(a): Parameters<LinksSearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let today = today();
        let items = links::read(&self.cfg);
        let here = self.resolve().ok().map(|c| c.repo);
        let tag = a.tag.map(|t| t.to_lowercase());
        let found: Vec<&links::Link> = links::ranked(&items, here.as_deref(), &today)
            .into_iter()
            .filter(|l| tag.as_ref().is_none_or(|t| l.tags.contains(t)))
            .filter(|l| a.query.as_ref().is_none_or(|q| links::matches_query(l, q)))
            .take(a.limit.unwrap_or(20))
            .collect();
        Ok(CallToolResult::structured(json!({ "items": found })))
    }

    #[tool(
        name = "bn_links_add",
        description = "Store one deliberately curated link. First use bn_links_search with the proposed category and a purpose query. Add only when the link is important, repeatedly useful, expensive to rediscover, and fills a gap no saved link already covers. A configured category is required; optional tags refine it. Do not save routine PRs, work items, review comments, search results, or every URL seen in a session. Re-adding the same page updates it in place; URL fragments do not create duplicates."
    )]
    async fn bn_links_add(
        &self,
        Parameters(a): Parameters<LinksAddArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let (repo, branch) = match a.repo.as_deref() {
            Some("") => (None, None),
            Some(r) => (Some(r.to_string()), None),
            None => match self.resolve() {
                Ok(c) => (Some(c.repo), Some(c.branch)),
                Err(_) => (None, None),
            },
        };
        let ttl_days = match a.ttl.as_deref() {
            Some(raw) => Some(links::parse_ttl(raw).ok_or_else(|| {
                ErrorData::invalid_params(
                    format!("ttl wants 7d, 2w or a day count, got '{raw}'"),
                    None,
                )
            })?),
            None => None,
        };
        let out = {
            let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
            let vocab = bn_core::resolver::link_tags();
            let category = links::validate_category(&a.category, &vocab)
                .map_err(|e| ErrorData::invalid_params(e.message(), None))?;
            let mut tags = a.tags.clone();
            tags.retain(|tag| !tag.eq_ignore_ascii_case(&category));
            tags.insert(0, category);
            let link = links::add(
                &self.cfg,
                &a.url,
                a.name.as_deref(),
                &tags,
                repo.as_deref(),
                branch.as_deref(),
                &today(),
            )
            .map_err(|e| ErrorData::invalid_params(e.message(), None))?;
            match ttl_days {
                Some(days) => links::set_expiry(&self.cfg, &link.id, &today(), days)
                    .map_err(|e| ErrorData::invalid_params(e.message(), None))?,
                None => link,
            }
        };
        Ok(CallToolResult::structured(json!({
            "id": out.id,
            "url": out.url,
            "name": out.name,
            "tags": out.tags,
            "repo": out.repo,
            "expires": out.expires,
        })))
    }

    #[tool(
        name = "bn_ref",
        description = "Reference docs: this repo's durable knowledge and global coding standards. Call with no name first for the index, then read the document relevant to your task rather than re-deriving the same facts."
    )]
    async fn bn_ref(
        &self,
        Parameters(a): Parameters<RefArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use bn_core::references::{self, Scope as RefScope};
        let ctx = self.resolve()?;
        let want = a.scope.as_deref().unwrap_or("all");
        let scopes: Vec<RefScope> = match want {
            "repo" => vec![RefScope::repo(&ctx.repo)],
            "global" => vec![RefScope::Global],
            "all" => vec![RefScope::repo(&ctx.repo), RefScope::Global],
            other => {
                return Err(ErrorData::invalid_params(
                    format!("unknown scope '{other}' -- use repo, global, or all"),
                    None,
                ));
            }
        };

        if let Some(query) = a.name.as_deref() {
            let (name, inline) = references::split_ref(query);
            let section = a.section.as_deref().or(inline);
            for scope in &scopes {
                if let Some((r, body)) = references::read_in(scope, name) {
                    let sections: Vec<_> = references::sections(&body)
                        .into_iter()
                        .map(
                            |(h, t)| json!({"heading": h, "tokens": bn_core::tokens::estimate(&t)}),
                        )
                        .collect();
                    let body = match section {
                        None => body,
                        Some(sec) => references::section_of(&body, sec).ok_or_else(|| {
                            ErrorData::invalid_params(
                                format!("{} has no section '{sec}' -- see `sections`", r.name),
                                None,
                            )
                        })?,
                    };
                    return Ok(CallToolResult::structured(json!({
                        "scope": scope.label(),
                        "name": r.name,
                        "section": section,
                        "summary": r.summary,
                        "path": r.path.display().to_string(),
                        "tokens": bn_core::tokens::estimate(&body),
                        "sections": sections,
                        "body": body,
                    })));
                }
            }
            return Err(ErrorData::invalid_params(
                format!(
                    "no reference matches '{name}' -- call bn_ref with no name to see what exists"
                ),
                None,
            ));
        }

        let mut references_out = Vec::new();
        for scope in &scopes {
            for r in references::list_in(scope) {
                let tokens = std::fs::read_to_string(&r.path)
                    .map(|b| bn_core::tokens::estimate(&b))
                    .unwrap_or(0);
                references_out.push(json!({
                    "scope": scope.label(),
                    "name": r.name,
                    "summary": r.summary,
                    "tokens": tokens,
                }));
            }
        }
        Ok(CallToolResult::structured(json!({
            "repo": ctx.repo,
            "count": references_out.len(),
            "references": references_out,
        })))
    }

    #[tool(
        name = "bn_cmd",
        description = "List or run the user's reusable shell commands in this worktree. Returns exit_code, stdout and stderr. Check available commands before scripting the same steps by hand. Global commands work even outside a worktree."
    )]
    async fn bn_cmd(
        &self,
        Parameters(a): Parameters<CmdArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use bn_core::cmds;
        let ctx = resolver::resolve(&self.cfg, &self.worktree).ok();
        let Some(name) = a.name.filter(|n| !n.trim().is_empty()) else {
            return Ok(CallToolResult::structured(json!({
                "commands": cmds::list(&self.cfg, ctx.as_ref()),
            })));
        };
        let Some(cmd) = cmds::find(&self.cfg, ctx.as_ref(), &name) else {
            return Err(ErrorData::invalid_params(
                format!("no command '{name}' -- call bn_cmd with no name to list them"),
                None,
            ));
        };
        if cfg!(windows) {
            return Err(ErrorData::invalid_params(
                "commands are sh scripts; not supported on native Windows",
                None,
            ));
        }
        let timeout = std::time::Duration::from_secs(a.timeout_secs.unwrap_or(120).clamp(1, 600));
        let command = cmds::command(&cmd, &a.args, &self.worktree, ctx.as_ref());
        let out = tokio::task::spawn_blocking(move || cmds::run_captured(command, timeout))
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .map_err(|e| ErrorData::internal_error(format!("failed to run {name}: {e}"), None))?;
        Ok(CallToolResult::structured(json!({
            "name": cmd.name,
            "scope": cmd.scope.label(),
            "exit_code": out.code,
            "timed_out": out.timed_out,
            "stdout": out.stdout,
            "stderr": out.stderr,
        })))
    }

    #[tool(
        name = "bn_ref_append",
<<<<<<< HEAD
        description = "Record one durable fact in this repo's reference docs. Include its source and verification date. Creates the document if necessary; returns appended:false for an exact duplicate. Session history belongs in Entire, not reference docs."
=======
        description = "Record ONE durable fact in this repo's reference notes, as a bullet under a section (facts by default, or gotchas). Use it the moment you learn something that will still be true next month and would save the next session time -- a root cause, a build gotcha, where a subsystem actually lives. Creates the note if it doesn't exist. Include where the fact came from and today's date in the text. Returns appended:false when that exact fact is already recorded. Task-specific context belongs in workspan task doc, not here."
>>>>>>> retire-notes-runtime
    )]
    async fn bn_ref_append(
        &self,
        Parameters(a): Parameters<RefAppendArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use bn_core::references::{self, Scope as RefScope};
        let ctx = self.resolve()?;
        if a.text.trim().is_empty() {
            return Err(ErrorData::invalid_params("nothing to record", None));
        }
        let scope = RefScope::repo(&ctx.repo);
        let out = {
            let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
            references::append(&scope, &a.name, a.section.as_deref(), &a.text)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        Ok(CallToolResult::structured(json!({
            "scope": scope.label(),
            "name": a.name,
            "heading": out.heading,
            "path": out.path.display().to_string(),
            "appended": out.appended,
        })))
    }
}

#[tool_handler]
impl ServerHandler for BnServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("bn-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(
<<<<<<< HEAD
            "Workspan MCP serves reference docs, reusable commands, curated links and workflow \
             prompts. Branch notes and their context, item, sync and handoff tools are retired. \
             Entire captures session history; repository onboarding must configure it with an \
             explicitly approved private checkpoint destination. Start from Git/workspace context \
             and the existing task and agent registries, not a branch note. \
             Use bn_ref with no name to read the reference index before rediscovering repository \
             facts. Use bn_ref_append for one durable fact with its source and verification date. \
             Use bn_cmd to discover and run reusable commands in this worktree. \
             Link tools work even outside a worktree: search the category and purpose with \
             bn_links_search, then use bn_links_add only for an important, repeatedly useful link \
             that fills a real gap. Do not save routine PRs, work items or review comments. \
             The default MCP cannot target unrelated worktrees. Use the Workspan CLI for explicit \
             task, machine and agent operations. Every effective skill is served as a prompt; \
             prompts provide instructions and do not grant additional permissions."
=======
            "Workspan serves repo references, curated links, user commands and workflow prompts. \
             Use workspan task doc/task do for task context and execution, workspan plan status/path \
             for plan artifacts, and workspan pipeline for execution state. Entire owns session history. \
             Read bn_ref without a name for the reference index; append only durable repo facts. \
             Search bn_links_search before adding an important, repeatedly useful link with bn_links_add. \
             bn_cmd lists and runs configured commands."
>>>>>>> retire-notes-runtime
                .to_string(),
        )
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let prompts = self
            .skills()?
            .iter()
            .map(crate::prompts::to_prompt)
            .collect();
        Ok(ListPromptsResult::with_all_items(prompts))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        let skills = self.skills()?;
        let skill = skills
            .iter()
            .find(|s| s.name == request.name)
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("unknown prompt: {}", request.name), None)
            })?;
        let lessons = bn_core::learnings::recent_lessons(&self.cfg, &today());
        let body = skill.render(request.arguments.as_ref())
            + &bn_core::learnings::lessons_section(&lessons, &skill.name);
        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, body)])
                .with_description(skill.description.clone()),
        )
    }
}
