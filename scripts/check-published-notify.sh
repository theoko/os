#!/usr/bin/env bash
# Run the published-download check on a timer and speak up only when it matters.
#
# A monitor whose entire output is a log file is a monitor nobody reads, and a
# monitor that reports every blip gets muted — after which it reports nothing
# at all. So:
#
#   * a failing run raises a macOS notification, because that is the only
#     channel anyone actually sees
#   * consecutive identical failures notify once, not every hour: the second
#     alert for a problem you already know about is what teaches you to ignore
#     the first
#   * recovery notifies too, so silence always means "fine", never "gave up"
#
# Every run appends to the log regardless, so there is a history to read after
# the fact.
set -uo pipefail

# Resolve the checker as a SIBLING, and keep every path out of ~/Desktop.
#
# launchd agents cannot read ~/Desktop, ~/Documents or ~/Downloads without
# Full Disk Access: the job loads, runs, and dies with "Operation not
# permitted" (exit 126) into a log it also cannot write — so it looks
# installed and does nothing. `check-published-install` therefore copies both
# scripts somewhere unprotected and points the timer at that copy.
HERE="$(cd "$(dirname "$0")" && pwd)"
CHECK="${CHECK_SCRIPT:-$HERE/check-published.sh}"

LOG="${CHECK_LOG:-$HOME/Library/Logs/os-check-published.log}"
STATE="${CHECK_STATE:-$HERE/state}"
mkdir -p "$(dirname "$LOG")"
STAMP="$(date '+%Y-%m-%d %H:%M:%S')"

notify() {
  # Failing to notify must never fail the check itself.
  osascript -e "display notification \"$2\" with title \"$1\"" >/dev/null 2>&1 || true
}

out="$(bash "$CHECK" --quiet 2>&1)"
rc=$?

prev="none"
[ -f "$STATE" ] && prev="$(cat "$STATE" 2>/dev/null || echo none)"

if [ "$rc" -eq 0 ]; then
  printf '[%s] ok\n' "$STAMP" >> "$LOG"
  if [ "$prev" = "failing" ]; then
    notify "teddy OS downloads" "Recovered — the published download is healthy again."
  fi
  echo ok > "$STATE"
  exit 0
fi

printf '[%s] FAILED\n%s\n' "$STAMP" "$out" >> "$LOG"

# The first reported problem is the most specific thing we know.
first="$(printf '%s' "$out" | grep -m1 'FAIL' | sed 's/^ *FAIL *//')"
[ -n "$first" ] || first="the published download check failed"

if [ "$prev" != "failing" ]; then
  notify "teddy OS downloads" "$first"
fi
echo failing > "$STATE"
exit 1
