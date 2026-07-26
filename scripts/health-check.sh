#!/usr/bin/env bash
# What is actually working, right now, in one page.
#
# Written because this repo has more than one session editing it and things
# quietly stopped working several times in a day: the tree went red mid-review
# four times, a 65MB portal corpus was deleted by a background task, a bridge
# kept serving a build from an hour earlier, and a UI redesign silently broke
# every driven test. None of those announced themselves.
#
# Fast checks only - no VM boots. Under a minute, so it can run on a schedule
# and be believed. Prints a line per check and exits non-zero if any FAILed.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:$PATH"

FAILED=0
pass() { printf '  PASS  %s\n' "$1"; }
fail() { printf '  FAIL  %s — %s\n' "$1" "$2"; FAILED=1; }
warn() { printf '  WARN  %s — %s\n' "$1" "$2"; }

echo "== build =="
for target in x86_64-unknown-none aarch64-unknown-none; do
  if out=$(cargo build -p kernel --target "$target" 2>&1); then
    pass "kernel builds for $target"
  else
    fail "kernel builds for $target" "$(grep -m1 '^error' <<<"$out")"
  fi
done
if out=$(cargo build -p os-mcp-bridge 2>&1); then pass "bridge builds"
else fail "bridge builds" "$(grep -m1 '^error' <<<"$out")"; fi

echo "== tests =="
for pkg in "kernel --lib" "os-core" "os-mcp-bridge"; do
  name="${pkg%% *}"
  if out=$(cargo test -p $pkg 2>&1); then
    pass "$name tests ($(grep -oE '[0-9]+ passed' <<<"$out" | head -1))"
  else
    fail "$name tests" "$(grep -m1 -E 'FAILED|^error' <<<"$out")"
  fi
done

echo "== working tree =="
dirty=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
if [[ "$dirty" -eq 0 ]]; then
  pass "no uncommitted changes"
else
  # Not a failure: another session may legitimately be mid-edit. But a tree
  # that has been dirty for days is how work gets lost.
  warn "$dirty uncommitted path(s)" "$(git status --porcelain | head -3 | tr '\n' ' ')"
fi

echo "== bridge =="
if nc -z 127.0.0.1 7420 2>/dev/null; then
  reply=$(printf 'PING\n' | nc -w 5 127.0.0.1 7420 2>/dev/null | head -1)
  if [[ "$reply" == *pong* ]]; then pass "bridge answers PING"
  else fail "bridge answers PING" "got: ${reply:-nothing}"; fi

  # The end-to-end path a guest actually uses. Built-in corpus, so it needs no
  # optional cached data.
  rows=$(BRIDGE_ADDR=127.0.0.1:7420 ./linux/agent-shell capability 2>/dev/null | grep -cE '^ *[0-9]+\. ')
  if [[ "${rows:-0}" -gt 0 ]]; then pass "agent-shell returns $rows rows"
  else fail "agent-shell returns rows" "the shipped client got nothing back"; fi

  status=$(printf 'CALL portal.status\n' | nc -w 5 127.0.0.1 7420 2>/dev/null | head -1)
  n=$(sed -n 's/.*n=\([0-9]*\).*/\1/p' <<<"$status")
  if [[ "${n:-0}" -gt 0 ]]; then pass "teddy corpus cached ($n docs)"
  else warn "teddy corpus" "not synced — portal queries will return nothing"; fi
else
  warn "bridge" "not running (start: ./scripts/ensure-bridge.sh)"
fi

echo "== artifacts =="
for f in os.iso os-arm64.iso; do
  if [[ -f "$f" ]]; then
    age=$(( ($(date +%s) - $(stat -f%m "$f")) / 3600 ))
    pass "$f present (${age}h old)"
  else
    warn "$f" "missing — run make iso / make arm64-iso"
  fi
done

echo
if [[ "$FAILED" -eq 0 ]]; then echo "HEALTH OK"; else echo "HEALTH DEGRADED"; fi
exit "$FAILED"
