#!/usr/bin/env bash
# guard-git.sh — deny destructive git commands from Copilot (preToolUse) and Claude (PreToolUse).
#
# Reads the hook payload on stdin. Copilot sends {toolName, toolArgs}; Claude sends
# {tool_name, tool_input}. Denies force-push, reset --hard, branch -D, clean -f and bare stash.
# Escape hatch: BN_GUARD_OFF=1 (bn pool cleanup legitimately resets worktrees).
set -u

[ "${BN_GUARD_OFF:-}" = 1 ] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

payload=$(cat)
cmd=$(printf '%s' "$payload" | jq -r '
  (.tool_input.command // (.toolArgs | if type == "string" then (try fromjson catch {}) else . end | .command) // empty)' 2>/dev/null)
[ -n "$cmd" ] || exit 0

# True when some `tmux` invocation in the command would create a session without `-s`: a bare
# `tmux` (its default command is new-session) or `new-session`/`new` lacking the flag.
unnamed_tmux_session() {
  local seg rest
  while IFS= read -r seg; do
    # tmux in command position: segment start (after env/exec/VAR= prefixes), `$(`, or a quote
    # opening `sh -c "tmux …"`. Mentions like `which tmux` or `grep tmux` are not invocations.
    [[ $seg =~ (^[[:space:]]*(\$\(|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+|(exec|command|env|timeout[[:space:]]+[0-9]+)[[:space:]]+|-u[[:space:]]+[^[:space:]]+[[:space:]]+)*|\$\(|[\"\'])tmux([[:space:]]+(.*))?$ ]] || continue
    rest=${BASH_REMATCH[5]:-}
    # Drop tmux's own global flags (-L/-S/-f/-T take a value) to reach the subcommand.
    while [[ $rest =~ ^(-[LSfT][[:space:]]+[^[:space:]]+|-[2CDlNuVv]+)([[:space:]]+(.*))?$ ]]; do
      rest=${BASH_REMATCH[3]:-}
    done
    if [[ -z ${rest//[[:space:]]/} || $rest =~ ^(new-session|new)([[:space:]]|$) ]]; then
      [[ $rest =~ (^|[[:space:]])-[A-Za-z]*s([[:space:]]|=|$) ]] || return 0
    fi
  done < <(printf '%s\n' "$cmd" | sed -E 's/\\;|;|&&|\|\||\|/\n/g')
  return 1
}

reason=""
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?push([[:space:]].*)?[[:space:]](-f|--force|--force-with-lease(=[^[:space:]]*)?|--force-if-includes)([[:space:]]|$)'; then
  reason="force-push"
elif printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?push([[:space:]].*)?[[:space:]]\+[^[:space:]]'; then
  reason="force-push (+refspec)"
elif printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?reset([[:space:]].*)?[[:space:]]--hard'; then
  reason="reset --hard"
elif printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?branch([[:space:]].*)?[[:space:]]-D([[:space:]]|$)'; then
  reason="branch -D"
elif printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?clean([[:space:]].*)?[[:space:]]-[a-z]*f'; then
  reason="clean -f"
elif printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?stash([[:space:]]+(push|pop|drop|clear|apply))?[[:space:]]*($|[;&|])'; then
  reason="bare git stash (shared across worktrees)"
elif unnamed_tmux_session; then
  reason="tmux session without -s (tmux would number it). Name it after the repo: tmux new-session -d -s <repo>"
fi
[ -n "$reason" ] || exit 0

msg="Blocked by bn guard: $reason. Ask the user, or set BN_GUARD_OFF=1 for an approved run."
if printf '%s' "$payload" | jq -e 'has("toolName")' >/dev/null 2>&1; then
  jq -n --arg r "$msg" '{permissionDecision: "deny", permissionDecisionReason: $r}'
else
  jq -n --arg r "$msg" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $r}}'
fi
exit 0
