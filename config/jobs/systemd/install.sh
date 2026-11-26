#!/bin/sh
# Bootstrap the workspan systemd user timers. This is a thin wrapper: it locates the installed workspan and
# defers to `workspan cron register`, which is the single source of truth on every platform. That reads
# config/crons.toml (+ any per-repo crons.toml) and generates + `daemon-reload`s + `enable --now`s a
# .service/.timer for **every** job — not a hand-maintained subset — each `Persistent=true` so a
# run missed while WSL was down catches up on next boot, then prunes any workspan timer no longer in the
# manifest. Idempotent: re-run any time to reconcile. Linux + systemd only (e.g. systemd-enabled
# WSL). On WSL these fire reliably because the single Windows keepalive task
# (config/jobs/windows/install.ps1) holds WSL up — see config/jobs/README.md.
set -eu

if ! command -v systemctl >/dev/null 2>&1; then
  echo "bn-systemd: systemctl not found — this installer is for systemd hosts only." >&2
  echo "  On stock WSL (no systemd) drive 'workspan run <job>' from Windows Task Scheduler;" >&2
  echo "  see config/jobs/README.md." >&2
  exit 1
fi

# ~/.config/bn/workspan is the stable, bindir-independent anchor the top-level install.sh links to the
# real binary. Fall back to a `workspan` on PATH if the anchor isn't there yet.
bn_bin="$HOME/.config/bn/workspan"
if [ ! -x "$bn_bin" ]; then
  if command -v workspan >/dev/null 2>&1; then
    bn_bin="$(command -v workspan)"
  else
    echo "bn-systemd: workspan not found (looked for $HOME/.config/bn/workspan and \`workspan\` on PATH)." >&2
    echo "  Install workspan first (run ./install.sh), then re-run this script." >&2
    exit 1
  fi
fi

"$bn_bin" cron register

echo "bn-systemd: registered via \`workspan cron register\`. Next runs:"
systemctl --user list-timers 'bn-*' --no-pager
