#!/usr/bin/env bash

bn_install_ws_alias() {
  local bindir="$1"
  if [ -L "$bindir/ws" ] && [ "$(readlink "$bindir/ws")" = workspan ]; then
    return 0
  fi
  if command -v ws >/dev/null 2>&1 || [ -e "$bindir/ws" ] || [ -L "$bindir/ws" ]; then
    echo "note: ws already exists; leaving it unchanged" >&2
  else
    ln -s workspan "$bindir/ws" || return 1
  fi
}

bn_install_workspan() {
  local bindir="$1" cfgdir="$2" target candidate link
  local -a path_dirs
  target="$bindir/workspan"
  if [ -e "$target" ] || [ -L "$target" ]; then
    if [ ! -L "$target" ] || [ "$(readlink "$target")" != bn ]; then
      if [ -L "$target" ]; then
        link="$(readlink "$target")"
        case "$link" in /*) ;; *) link="$bindir/$link" ;; esac
      else
        link=""
      fi
      if [ -z "$link" ] || [ ! -L "$link" ] || [ "$(readlink "$link")" != bn ]; then
        echo "error: refusing to replace existing $target with Workspan" >&2
        return 1
      fi
      ln -sfn bn "$target" || return 1
    fi
  else
    ln -s bn "$target" || return 1
  fi
  if [ -e "$cfgdir/workspan" ] || [ -L "$cfgdir/workspan" ]; then
    if [ ! -L "$cfgdir/workspan" ]; then
      echo "error: refusing to replace existing $cfgdir/workspan anchor" >&2
      return 1
    fi
    link="$(readlink "$cfgdir/workspan")"
    case "$link" in /*) ;; *) link="$cfgdir/$link" ;; esac
    if [ "$link" != "$target" ]; then
      if [ ! -L "$link" ] || [ "$(readlink "$link")" != bn ]; then
        echo "error: refusing to replace existing $cfgdir/workspan anchor" >&2
        return 1
      fi
      ln -sfn "$target" "$cfgdir/workspan" || return 1
    fi
  else
    ln -s "$target" "$cfgdir/workspan" || return 1
  fi
  bn_install_ws_alias "$bindir" || return 1
  # A branch build lives outside PATH. Follow only the bn links the installer just repointed.
  IFS=':' read -r -a path_dirs <<< "$PATH"
  for candidate in "${path_dirs[@]}" "$HOME/.bin"; do
    [ -d "$candidate" ] || continue
    candidate="$(cd "$candidate" && pwd -P)" || continue
    case "$candidate" in "$HOME"/*) ;; *) continue ;; esac
    [ "$candidate" != "$bindir" ] || continue
    [ -L "$candidate/bn" ] && [ "$(readlink "$candidate/bn")" = "$bindir/bn" ] || continue
    if [ -e "$candidate/workspan" ] || [ -L "$candidate/workspan" ]; then
      link="$(readlink "$candidate/workspan")" || continue
      case "$link" in /*) ;; *) link="$candidate/$link" ;; esac
      if [ "$link" != "$candidate/bn" ] && [ "$link" != "$target" ]; then
        [ -L "$link" ] && [ "$(readlink "$link")" = bn ] || continue
      fi
    fi
    ln -sfn "$target" "$candidate/workspan" || return 1
    bn_install_ws_alias "$candidate" || return 1
  done
  if [ "${WORKSPAN_INSTALL_WP:-0}" = 1 ]; then
    if command -v wp >/dev/null 2>&1 || [ -e "$bindir/wp" ] || [ -L "$bindir/wp" ]; then
      echo "note: wp already exists; leaving it unchanged" >&2
    else
      ln -s workspan "$bindir/wp" || return 1
    fi
  fi
}
