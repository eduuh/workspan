#!/usr/bin/env bash
# Rename a tmux session tmux auto-named with a number (plain `tmux`, `new-session` without `-s`)
# after the repo its first pane started in. Wired to the `session-created` hook in tmux.conf so
# every session reads as a repo, never `5`, `6`, `7`.
#
#   tmux-name-session.sh <session_name> <pane_path>
#
# - Named sessions are left alone; only all-digit names are touched.
# - Outside any repo the session is called `home`.
# - If a session with that name already exists, attached clients move to it and the numbered
#   duplicate is killed, so the repo keeps exactly one session.
set -u

name=$1 path=${2:-$HOME}

[[ $name =~ ^[0-9]+$ ]] || exit 0
id="=$name"

repo_name() {
  local common
  common=$(git -C "$1" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return 1
  case $common in
    */.git) common=${common%/.git} ;;  # regular clone: name the work tree
  esac
  common=${common%.git}               # bare repo: workspan.git -> workspan
  basename "$common"
}

target=$(repo_name "$path") || target=home
target=${target//[.:]/-}  # tmux treats . and : as target separators

if tmux has-session -t "=$target" 2>/dev/null; then
  tmux list-clients -t "$id" -F '#{client_name}' 2>/dev/null | while read -r c; do
    tmux switch-client -c "$c" -t "=$target"
  done
  tmux kill-session -t "$id"
else
  tmux rename-session -t "$id" -- "$target"
fi
