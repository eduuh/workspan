#!/bin/sh
set -eu

pane=${1:-${TMUX_PANE:-}}
if [ -z "$pane" ]; then
  echo "tmux-git-diff: no tmux pane supplied" >&2
  exit 1
fi

path=$(tmux display-message -p -t "$pane" '#{pane_current_path}')
if git -C "$path" rev-parse --verify --quiet origin/main >/dev/null; then
  base_ref=origin/main
elif git -C "$path" rev-parse --verify --quiet main >/dev/null; then
  base_ref=main
else
  echo "tmux-git-diff: neither origin/main nor main exists" >&2
  exit 1
fi

base=$(git -C "$path" merge-base "$base_ref" HEAD)
if git -C "$path" diff --quiet "$base"; then
  # Nothing against main (e.g. already merged): review the latest commit instead.
  rev="HEAD~1"
else
  rev="$base_ref"
fi

need="$(dirname "$0")/tmux-need.sh"

# Opt-in (`set -g @bn_diff_float on`, tmux 3.8+): open the editor as a modal floating PANE instead
# of a popup. A popup belongs to the client and dies with it; a pane belongs to the window, so you
# can switch to another window and come back to the diff where you left it, and it has copy mode.
# Its id is recorded on the window so the pane-exited hook does not badge the window "done" when
# the editor closes. workspan addresses the agent's pane past a float (`under_float`), so messages
# and state reads still reach the agent while this is open.
if [ "$(tmux show-options -gqv @bn_diff_float)" = on ] &&
   [ "$(tmux display-message -p '#{>=:#{version},3.8}')" = 1 ]; then
  if id=$(tmux new-pane -O -P -F '#{pane_id}' -t "$pane" -x 97% -y 97% -c "$path" \
      -T "diff vs $rev" "'$need' nvim -c 'DiffviewOpen $rev'"); then
    tmux set-option -w -t "$pane" @bn_diff_pane "$id"
    tmux move-pane -P centre -t "$id"
    exit 0
  fi
fi

# Detach so an agent calling this script is not blocked until the popup closes.
tmux run-shell -b -t "$pane" "tmux display-popup -E -t '$pane' -w 97% -h 97% -d '$path' \"'$need' nvim -c 'DiffviewOpen $rev'\""
