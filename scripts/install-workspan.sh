#!/usr/bin/env bash
# Links around the installed `workspan` binary: the stable config-dir anchor, the `ws` alias (and
# the opt-in `wp` one), and the retirement of the `bn` names older installs left behind.

# `ws` -> workspan, unless something else already answers to `ws`.
workspan_install_alias() {
  local bindir="$1" alias="$2"
  if [ -L "$bindir/$alias" ] && [ "$(readlink "$bindir/$alias")" = workspan ]; then
    return 0
  fi
  if command -v "$alias" >/dev/null 2>&1 || [ -e "$bindir/$alias" ] || [ -L "$bindir/$alias" ]; then
    echo "note: $alias already exists; leaving it unchanged" >&2
  else
    ln -s workspan "$bindir/$alias" || return 1
  fi
}

# Whether $1 is a workspan build that older installs placed under the `bn` name.
workspan_is_legacy_bn() {
  local file="$1"
  [ -f "$file" ] && [ -x "$file" ] || return 1
  "$file" --version 2>/dev/null | grep -qE '^(bn|workspan) [0-9]'
}

# Remove what older installs put under the `bn` names. Only removes what is provably ours: a
# binary (or link to one) that reports itself as bn/workspan. A user's own `bn` stays.
workspan_retire_bn() {
  local bindir="$1" cfgdir="$2" link candidate name
  if [ -L "$cfgdir/bn" ] && { [ ! -e "$cfgdir/bn" ] || workspan_is_legacy_bn "$cfgdir/bn"; }; then
    rm -f "$cfgdir/bn"
    echo "Removed the retired $cfgdir/bn anchor"
  fi
  if workspan_is_legacy_bn "$bindir/bn"; then
    rm -f "$bindir/bn"
    echo "Removed the retired $bindir/bn binary"
  fi
  # bn-mcp only ever came from this installer; drop it once no bn is left beside it.
  if [ ! -e "$bindir/bn" ] && { [ -e "$bindir/bn-mcp" ] || [ -L "$bindir/bn-mcp" ]; }; then
    rm -f "$bindir/bn-mcp"
  fi
  if [ -L "$bindir/bn-setup-entire" ] ||
    { [ -f "$bindir/bn-setup-entire" ] && [ -f "$bindir/workspan-setup-entire" ] &&
      cmp -s "$bindir/bn-setup-entire" "$bindir/workspan-setup-entire"; }; then
    rm -f "$bindir/bn-setup-entire"
  fi
  # Older installs repointed shadowing copies on PATH at $bindir/bn. Drop those links.
  local -a path_dirs
  IFS=':' read -r -a path_dirs <<< "$PATH"
  for candidate in ${path_dirs[@]+"${path_dirs[@]}"} "$HOME/.bin"; do
    [ -d "$candidate" ] || continue
    case "$candidate" in "$HOME"/*) ;; *) continue ;; esac
    for name in bn bn-mcp; do
      [ -L "$candidate/$name" ] || continue
      link="$(readlink "$candidate/$name")"
      [ "$link" = "$bindir/$name" ] || continue
      rm -f "$candidate/$name"
      echo "Removed the retired $candidate/$name link"
    done
  done
}

# Point the stable anchor $cfgdir/workspan at $bindir/workspan, add the aliases, retire `bn`.
workspan_install_links() {
  local bindir="$1" cfgdir="$2" target
  target="$bindir/workspan"
  if [ ! -x "$target" ]; then
    echo "error: $target is missing or not executable; refusing to link $cfgdir/workspan." >&2
    return 1
  fi
  if [ -e "$cfgdir/workspan" ] && [ ! -L "$cfgdir/workspan" ]; then
    echo "error: refusing to replace existing $cfgdir/workspan anchor" >&2
    return 1
  fi
  ln -sfn "$target" "$cfgdir/workspan" || return 1
  workspan_install_alias "$bindir" ws || return 1
  if [ "${WORKSPAN_INSTALL_WP:-0}" = 1 ]; then
    workspan_install_alias "$bindir" wp || return 1
  fi
  workspan_retire_bn "$bindir" "$cfgdir"
}
