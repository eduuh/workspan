#!/usr/bin/env bash
# Tests guard-git.sh against both the Claude and Copilot payload shapes.
guard="$(cd "$(dirname "$0")/../bin" && pwd)/guard-git.sh"
fail=0

claude() { jq -n --arg c "$1" '{tool_name: "Bash", tool_input: {command: $c}}' | "$guard"; }
copilot() { jq -n --arg c "$1" '{toolName: "bash", toolArgs: ({command: $c} | tojson)}' | "$guard"; }

check() { # name expect(deny|allow) output
  local got=allow
  case "$3" in *'"deny"'*) got=deny ;; esac
  if [ "$got" != "$2" ]; then echo "FAIL: $1 expected $2 got $got"; fail=1; fi
}

for c in "git push --force origin main" "git push -f" "git push origin +main" \
  "git push --force-with-lease" "git reset --hard HEAD~1" "git -C /x reset --hard" \
  "git branch -D foo" "git clean -fd" "git stash" "git stash pop" "cd x && git stash"; do
  check "claude: $c" deny "$(claude "$c")"
  check "copilot: $c" deny "$(copilot "$c")"
done

for c in "git push origin feature" "git reset --soft HEAD~1" "git branch -d foo" \
  "git stash list" "git status" "ls -la" "git push --set-upstream origin x"; do
  check "claude: $c" allow "$(claude "$c")"
  check "copilot: $c" allow "$(copilot "$c")"
done

# tmux sessions must be named: an unnamed one gets a number instead of its repo.
for c in "tmux" "tmux new -d" "tmux new-session -d -c /tmp" "tmux -L x new-session -d" \
  "env -u TMUX tmux new-session -d" "cd x && tmux new -d" "bash -c \"tmux new-session -d\"" \
  "id=\$(tmux new-session -d -P)" "tmux kill-server; tmux new-session -d"; do
  check "claude: $c" deny "$(claude "$c")"
  check "copilot: $c" deny "$(copilot "$c")"
done

for c in "tmux new-session -d -s workspan" "tmux new -A -s main" "tmux new -ds repo" \
  "tmux new-session -d -c \"/x y\" -s repo" "tmux -L x new-session -d -s t" "tmux ls" \
  "tmux new-window -d -t workspan: -n build" "which tmux" "grep tmux file" \
  "ls ~/.bin/tmux/" "tmux has-session -t x || tmux new-session -d -s x"; do
  check "claude: $c" allow "$(claude "$c")"
  check "copilot: $c" allow "$(copilot "$c")"
done

check "escape hatch" allow "$(BN_GUARD_OFF=1 claude "git push --force")"
check "non-bash tool" allow "$(printf '{"tool_name":"Read","tool_input":{"file_path":"x"}}' | "$guard")"

[ "$fail" = 0 ] && echo "guard-git: all passed"
exit "$fail"
