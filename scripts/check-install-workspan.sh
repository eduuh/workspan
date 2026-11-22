#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source "$repo/scripts/install-workspan.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" PATH="$tmp/bin:/usr/bin:/bin"
mkdir -p "$tmp/bin" "$tmp/config" "$HOME"
printf '#!/bin/sh\nprintf "compatibility\\n"\n' > "$tmp/bin/bn"
chmod +x "$tmp/bin/bn"
bn_install_workspan "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/workspan")" = compatibility ]
[ "$("$tmp/config/workspan")" = compatibility ]
[ "$("$tmp/bin/ws")" = compatibility ]
[ "$(readlink "$tmp/bin/ws")" = workspan ]
[ ! -e "$tmp/bin/wp" ]
bn_install_workspan "$tmp/bin" "$tmp/config"

rm "$tmp/bin/ws"
printf '#!/bin/sh\necho unrelated-ws\n' > "$tmp/bin/ws"
chmod +x "$tmp/bin/ws"
bn_install_workspan "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/ws")" = unrelated-ws ]
rm "$tmp/bin/ws"
ln -s missing-command "$tmp/bin/ws"
bn_install_workspan "$tmp/bin" "$tmp/config"
[ "$(readlink "$tmp/bin/ws")" = missing-command ]
rm "$tmp/bin/ws"

WORKSPAN_INSTALL_WP=1 bn_install_workspan "$tmp/bin" "$tmp/config"
if [ -L "$tmp/bin/wp" ]; then
  [ "$(readlink "$tmp/bin/wp")" = workspan ]
fi
# Remove only the alias created by this isolated test before planting a real command.
if [ -L "$tmp/bin/wp" ]; then
  rm "$tmp/bin/wp"
fi
printf '#!/bin/sh\necho wordpress\n' > "$tmp/bin/wp"
chmod +x "$tmp/bin/wp"
WORKSPAN_INSTALL_WP=1 bn_install_workspan "$tmp/bin" "$tmp/config"
[ "$("$tmp/bin/wp")" = wordpress ]

rm "$tmp/bin/workspan"
printf 'unrelated tool\n' > "$tmp/bin/workspan"
if bn_install_workspan "$tmp/bin" "$tmp/config"; then
  echo "expected conflicting workspan command to be preserved" >&2
  exit 1
fi
[ "$(cat "$tmp/bin/workspan")" = 'unrelated tool' ]

rm "$tmp/bin/workspan" "$tmp/config/workspan"
printf 'unrelated anchor\n' > "$tmp/config/workspan"
if bn_install_workspan "$tmp/bin" "$tmp/config"; then
  echo "expected conflicting Workspan anchor to be preserved" >&2
  exit 1
fi
[ "$(cat "$tmp/config/workspan")" = 'unrelated anchor' ]

mkdir -p "$tmp/home/bin" "$tmp/home/build" "$tmp/home/config" "$tmp/home/old"
cp "$tmp/bin/bn" "$tmp/home/build/bn"
cp "$tmp/bin/bn" "$tmp/home/old/bn"
ln -s bn "$tmp/home/old/workspan"
ln -s "$tmp/home/old/workspan" "$tmp/home/config/workspan"
ln -s "$tmp/home/build/bn" "$tmp/home/bin/bn"
ln -s "$tmp/home/old/workspan" "$tmp/home/bin/workspan"
HOME="$tmp/home" PATH="$tmp/home/bin:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/build" "$tmp/home/config"
[ "$(readlink "$tmp/home/bin/workspan")" = "$tmp/home/build/workspan" ]
[ "$(readlink "$tmp/home/config/workspan")" = "$tmp/home/build/workspan" ]
[ "$("$tmp/home/bin/ws")" = compatibility ]
HOME="$tmp/home" PATH="$tmp/home/bin:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/build" "$tmp/home/config"

rm "$tmp/home/build/ws" "$tmp/home/bin/ws"
mkdir -p "$tmp/home/other"
printf '#!/bin/sh\necho external-ws\n' > "$tmp/home/other/ws"
chmod +x "$tmp/home/other/ws"
HOME="$tmp/home" PATH="$tmp/home/other:$tmp/home/bin:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/build" "$tmp/home/config"
[ ! -e "$tmp/home/build/ws" ] && [ ! -e "$tmp/home/bin/ws" ]
[ "$("$tmp/home/other/ws")" = external-ws ]

rm "$tmp/home/bin/workspan"
printf '#!/bin/sh\necho unrelated-workspan\n' > "$tmp/home/bin/workspan"
chmod +x "$tmp/home/bin/workspan"
HOME="$tmp/home" PATH="$tmp/home/bin:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/build" "$tmp/home/config"
[ "$("$tmp/home/bin/workspan")" = unrelated-workspan ]
mv "$tmp/home/bin/workspan" "$tmp/home/bin/unrelated-workspan"
ln -s unrelated-workspan "$tmp/home/bin/workspan"
HOME="$tmp/home" PATH="$tmp/home/bin:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/build" "$tmp/home/config"
[ "$(readlink "$tmp/home/bin/workspan")" = unrelated-workspan ]

mkdir -p "$tmp/home/direct" "$tmp/home/direct-config"
cp "$tmp/bin/bn" "$tmp/home/direct/bn"
ln -s "$tmp/home/old/workspan" "$tmp/home/direct/workspan"
HOME="$tmp/home" PATH="$tmp/home/direct:/usr/bin:/bin" \
  bn_install_workspan "$tmp/home/direct" "$tmp/home/direct-config"
[ "$(readlink "$tmp/home/direct/workspan")" = bn ]
[ "$("$tmp/home/direct/ws")" = compatibility ]
echo "check-install-workspan: ok"
