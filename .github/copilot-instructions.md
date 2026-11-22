<!-- BEGIN bn (managed by `bn mcp init`) -->

## Workspan agent integration

Workspan coordinates workspaces, agents, machines, and execution. Entire handles captured
agent-session history. The `bn` command and MCP tool names remain compatibility interfaces.

### Session capture and repository setup

Branch notes and their dependent tools are retired. Start from Git/workspace context and the
task and agent registries. Do not create branch notes or use them as a session ledger.

Repository onboarding must enable Entire for the selected harness through the approved setup
helper (`workspan-setup-entire copilot` or `workspan-setup-entire claude`). Checkpoint capture
requires an explicitly approved private destination; never default to uploading session history
alongside the code remote. Setup failure is an onboarding blocker.

### MCP tools

The client starts `bn mcp serve` over stdio in its worktree. The server offers:

- `bn_ref` and `bn_ref_append` — read repository/global reference docs and record durable facts.
  Read the index first; include sources and verification dates when recording facts.
- `bn_cmd` — discover and run reusable commands in the current worktree.
- `bn_links_search` and `bn_links_add` — search before saving a deliberately curated link.
  Save only important, repeatedly useful links that fill a real gap, not routine PRs or work items.

Use the Workspan CLI for task, machine and agent operations. MCP prompts are reusable workflows
from `config/prompts/`; Copilot installs them as native skills. Prompts supply instructions, not
permissions. Reference skills in `config/skills/` provide standards, not branch session state.

<!-- END bn -->
