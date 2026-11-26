#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source "$repo/scripts/install-workspan.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" PATH="$tmp/bin:/usr/bin:/bin"
mkdir -p "$tmp/bin" "$tmp/config" "$HOME"
fake() { printf '#!/bin/sh\necho "%s"\n' "$2" > "$1"; chmod +x "$1"; }

# A missing binary is never anchored.
if workspan_install_links "$tmp/bin" "$tmp/config" 2>/dev/null; then
  echo "expected a missing workspan binary to be refused" >&2
  exit 1
fi

fake "$tmp/bin/workspan" "workspan 9.9.9"
workspan_install_links "$tmp/bin" "$tmp/config"
[ "$(readlink "$tmp/config/workspan")" = "$tmp/bin/workspan" ]
[ "$(readlink "$tmp/bin/ws")" = workspan ]
[ ! -e "$tmp/bin/wp" ]
workspan_install_links "$tmp/bin" "$tmp/config"

# Someone else's ws, even a dangling one, is left alone.
rm "$tmp/bin/ws"
fake "$tmp/bin/ws" unrelated-ws
workspan_install_links "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/ws")" = unrelated-ws ]
rm "$tmp/bin/ws"
ln -s missing-command "$tmp/bin/ws"
workspan_install_links "$tmp/bin" "$tmp/config"
[ "$(readlink "$tmp/bin/ws")" = missing-command ]
rm "$tmp/bin/ws"

# wp is opt-in and never shadows an existing wp.
WORKSPAN_INSTALL_WP=1 workspan_install_links "$tmp/bin" "$tmp/config"
[ "$(readlink "$tmp/bin/wp")" = workspan ]
rm "$tmp/bin/wp"
fake "$tmp/bin/wp" wordpress
WORKSPAN_INSTALL_WP=1 workspan_install_links "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/wp")" = wordpress ]

# A real file at the anchor is never replaced.
rm "$tmp/config/workspan"
printf 'unrelated anchor\n' > "$tmp/config/workspan"
if workspan_install_links "$tmp/bin" "$tmp/config" 2>/dev/null; then
  echo "expected conflicting Workspan anchor to be preserved" >&2
  exit 1
fi
[ "$(cat "$tmp/config/workspan")" = 'unrelated anchor' ]
rm "$tmp/config/workspan"

# The bn names an older install left behind are retired.
mkdir -p "$HOME/.bin"
fake "$tmp/bin/bn" "bn 0.5.0"
fake "$tmp/bin/bn-mcp" "bn-mcp"
cp "$repo/config/workflow/bin/setup-entire.sh" "$tmp/bin/bn-setup-entire"
cp "$repo/config/workflow/bin/setup-entire.sh" "$tmp/bin/workspan-setup-entire"
ln -s "$tmp/bin/bn" "$tmp/config/bn"
ln -s "$tmp/bin/bn" "$HOME/.bin/bn"
workspan_install_links "$tmp/bin" "$tmp/config" >/dev/null
for gone in "$tmp/bin/bn" "$tmp/bin/bn-mcp" "$tmp/bin/bn-setup-entire" "$tmp/config/bn" "$HOME/.bin/bn"; do
  if [ -e "$gone" ] || [ -L "$gone" ]; then
    echo "expected $gone to be retired" >&2
    exit 1
  fi
done
[ -f "$tmp/bin/workspan-setup-entire" ]

# A bn that is not workspan stays, and so does the bn-mcp beside it.
fake "$tmp/bin/bn" "some other tool"
fake "$tmp/bin/bn-mcp" "bn-mcp"
workspan_install_links "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/bn")" = "some other tool" ]
[ -e "$tmp/bin/bn-mcp" ]
echo "check-install-workspan: ok"
