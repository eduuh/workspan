#!/bin/sh
# tmux-tunnels -- connected tunnels in the tmux status bar: `⇄ AB12C✓`.
#
# Reads the status file the tunnel-auto watchdog writes every minute (~/.cache/bn/tunnels.status,
# "<name> ok|down" per line) and never checks anything itself, so the bar stays instant. Names are
# shown by their last dash-separated part (CPC-me-AB12C → AB12C; the prefix says nothing).
# Only `ok` entries are shown. A file older than 10 minutes means the watchdog stopped, shown as
# `⇄ ?`. A machine with no connected tunnels prints nothing.
f="$HOME/.cache/bn/tunnels.status"
# tmux.conf passes its theme as BN_TMUX_THEME; light uses the Tokyo Night Day counterparts.
if [ "${BN_TMUX_THEME:-}" = light ]; then
  dim='#8990b3' green='#587539' blue='#2e7de9'
else
  dim='#565f89' green='#9ece6a' blue='#7aa2f7'
fi
[ -f "$f" ] || exit 0
# Stale when older than 10 minutes, or dated in the future (a WSL clock that jumped).
m=$(stat -c %Y "$f" 2>/dev/null || stat -f %m "$f" 2>/dev/null); age=$(( $(date +%s) - ${m:-0} ))
if [ "$age" -lt 0 ] || [ "$age" -ge 600 ]; then
  printf '#[fg=%s]⇄ ? ' "$dim"
  exit 0
fi
# A host's nickname (the second name on its `Host` line in tunnel-connect's block) wins over the
# hostname's last part.
nicks=$(awk '/^# >>> bn tunnel /{inb=1} inb && $1 == "Host" && NF >= 3 {print $2, $3} /^# <<< bn tunnel /{inb=0}' "$HOME/.ssh/config" 2>/dev/null)
out=""
while read -r name state; do
  [ -n "$name" ] || continue
  [ "$state" = ok ] || continue
  short=$(printf '%s\n' "$nicks" | awk -v n="$name" '$1 == n {print $2; exit}')
  [ -n "$short" ] || short=${name##*-}
  out="$out #[fg=$green]${short}✓"
done < "$f"
if [ -n "$out" ]; then
  printf '#[fg=%s]⇄%s ' "$blue" "$out"
fi
