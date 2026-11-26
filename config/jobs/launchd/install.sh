#!/bin/sh
# Bootstrap the workspan launchd user agents on macOS. This is a thin wrapper: it locates the installed
# workspan and defers to `workspan cron register`, which is the single source of truth on every platform. That
# reads config/crons.toml (+ any per-repo crons.toml) and renders + (re)bootstraps a launchd
# user-agent for **every** job — not a hand-maintained subset — then prunes any workspan agent no longer
# in the manifest (including the legacy static com.bn.refresh / com.bn.daily agents earlier
# versions of this script installed). Idempotent: re-run any time to reconcile. macOS only.
set -eu

if ! command -v launchctl >/dev/null 2>&1; then
  echo "bn-launchd: launchctl not found — this installer is for macOS." >&2
  echo "  On Linux/WSL use config/jobs/systemd/install.sh instead." >&2
  exit 1
fi

# ~/.config/bn/workspan is the stable, bindir-independent anchor the top-level install.sh links to the
# real binary. Fall back to a `workspan` on PATH if the anchor isn't there yet.
bn_bin="$HOME/.config/bn/workspan"
if [ ! -x "$bn_bin" ]; then
  if command -v workspan >/dev/null 2>&1; then
    bn_bin="$(command -v workspan)"
  else
    echo "bn-launchd: workspan not found (looked for $HOME/.config/bn/workspan and \`workspan\` on PATH)." >&2
    echo "  Install workspan first (run ./install.sh), then re-run this script." >&2
    exit 1
  fi
fi

"$bn_bin" cron register

echo "bn-launchd: registered via \`workspan cron register\`. Loaded agents:"
launchctl list | grep com.bn || true
