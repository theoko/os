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
#
# STANDALONE NOTE: host/bridge/ (os-mcp-bridge) was removed by the standalone
# refactor, and the kernel's guest query path returns Offline unconditionally
# in a standalone build (kernel/src/mcp.rs; CLAUDE.md: "No bridge probing on
# COM2"). The old "== bridge ==" section built/tested a package that no
# longer exists and pinged a process that has nothing to serve, so it's
# replaced below with "== search ==", the equivalent check for what a
# standalone guest actually has: the compile-time corpus baked from
# search/corpus.json.
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

echo "== tests =="
for pkg in "kernel --lib" "os-core"; do
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

echo "== search =="
# What a standalone guest actually has: a corpus baked into the kernel at
# compile time from search/corpus.json (kernel/src/search.rs, kernel/build.rs).
# There is no live index and nothing to ping — just a file to validate.
if [[ -f search/corpus.json ]]; then
  docs=$(python3 -c "import json,sys; print(len(json.load(open('search/corpus.json')).get('docs', [])))" 2>/dev/null || echo "")
  if [[ "$docs" =~ ^[0-9]+$ ]] && [[ "$docs" -gt 0 ]]; then
    pass "search/corpus.json parses ($docs docs)"
  else
    fail "search/corpus.json parses" "unreadable, not JSON, or has no docs"
  fi
else
  fail "search/corpus.json present" "missing — the kernel would bake an empty corpus"
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
