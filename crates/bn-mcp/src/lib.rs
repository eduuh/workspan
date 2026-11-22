//! bn-mcp — the Workspan Model Context Protocol surface for references, commands, links and
//! workflow prompts. Session history is captured by Entire.
//!
//! The [`server::BnServer`] tool-router is pinned to its launch worktree and served over stdio
//! by `bn mcp serve`. The separate [`orchestrator`] surface remains explicitly opt-in.

pub mod orchestrator;
mod prompts;
pub mod server;
