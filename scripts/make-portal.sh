#!/usr/bin/env bash
# One machine per tSearch portal, with each user's data kept apart.
#
# The ISO is portal-agnostic — the guest talks to a host bridge over COM2 and
# has no idea whose machine it is. Everything that differs between users lives
# on the host: which corpus to fetch, which credential to use, and where the
# fetched private data is cached. So this builds a per-portal BRIDGE PROFILE,
# not a per-portal kernel.
#
# THE POINT OF THE PROFILE
# Every path the bridge writes is derived from $HOME:
#
#   config/config.json          knowledge/teddy.json      knowledge/emails.json
#   knowledge/portals.json      knowledge/workspace.json  knowledge/transcripts.json
#   skills/
#
# Left at the default, running portal A and then portal B would serve A's
# cached corpus, A's indexed mail and A's transcripts to B — one person's
# private documents showing up on another person's machine. Giving each portal
# its own HOME isolates all seven at once, with no change to the bridge.
#
#   ./scripts/make-portal.sh theo          # start a bridge for that portal
#   ./scripts/make-portal.sh theo --sync   # ...and fetch its corpus
#   ./scripts/make-portal.sh --list        # what is configured
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# Same PATH the Makefile builds with; cargo is not on a bare login PATH here.
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:$PATH"

REGISTRY="$ROOT/portals.json"
PROFILES="${OS_PORTAL_PROFILES:-$HOME/Library/Application Support/os-portals}"
BASE_PORT="${OS_PORTAL_BASE_PORT:-7500}"

[[ -f "$REGISTRY" ]] || { echo "error: $REGISTRY missing" >&2; exit 1; }

portal_field() {
  python3 - "$REGISTRY" "$1" "$2" <<'PY'
import json, sys
reg, name, field = sys.argv[1], sys.argv[2], sys.argv[3]
for p in json.load(open(reg))["portals"]:
    if p["name"] == name:
        print(p.get(field, ""))
        break
PY
}

list_portals() {
  python3 - "$REGISTRY" "$PROFILES" <<'PY'
import json, os, sys
reg, profiles = sys.argv[1], sys.argv[2]
print(f"{'portal':<12} {'auth':<7} {'corpus cached':<14} corpus")
for p in json.load(open(reg))["portals"]:
    home = os.path.join(profiles, p["name"], "home")
    cache = os.path.join(home, "Library/Application Support/os/knowledge/teddy.json")
    if os.path.exists(cache):
        mb = os.path.getsize(cache) / 1024 / 1024
        cached = f"{mb:.0f} MB"
    else:
        cached = "-"
    print(f"{p['name']:<12} {p['auth']:<7} {cached:<14} {p['corpus']}")
PY
}

if [[ "${1:-}" == "--list" || -z "${1:-}" ]]; then
  list_portals
  echo
  echo "start one:  ./scripts/make-portal.sh <portal> [--sync]"
  exit 0
fi

NAME="$1"; shift
SYNC=0
for arg in "$@"; do [[ "$arg" == "--sync" ]] && SYNC=1; done

CORPUS="$(portal_field "$NAME" corpus)"
AUTH="$(portal_field "$NAME" auth)"
[[ -n "$CORPUS" ]] || { echo "error: no portal '$NAME' in $REGISTRY" >&2; list_portals >&2; exit 1; }

# A stable per-portal port, so several can run side by side and each guest's
# COM2 reaches its own bridge rather than whichever started last.
PORT=$(( BASE_PORT + $(python3 -c "
import json,sys
names=[p['name'] for p in json.load(open('$REGISTRY'))['portals']]
print(names.index('$NAME'))
") ))
PROFILE="$PROFILES/$NAME/home"
mkdir -p "$PROFILE/Library/Application Support/os/knowledge"

echo "portal:   $NAME"
echo "corpus:   $CORPUS"
echo "profile:  $PROFILE"
echo "bridge:   127.0.0.1:$PORT"

if [[ "$AUTH" == "basic" ]]; then
  # Never handle the password. Check for the credential and print the exact
  # command if it is missing; the value goes from the operator to the keychain
  # without passing through this script, its argv, or any log.
  if HOME="$PROFILE" security find-generic-password -s os-portal -a "$NAME" >/dev/null 2>&1; then
    echo "auth:     basic, credential found in keychain"
  else
    echo "auth:     basic, NO CREDENTIAL STORED"
    echo
    echo "  This portal needs one before it can sync. Run this yourself:"
    echo "      security add-generic-password -s os-portal -a $NAME -w"
    echo "  (it will prompt for the password; it is not passed on a command line)"
    [[ "$SYNC" == "1" ]] && { echo; echo "refusing --sync without a credential"; exit 1; }
  fi
fi

cargo build -p os-mcp-bridge >/dev/null 2>&1 || { echo "error: bridge build failed" >&2; exit 1; }

pkill -f "os-mcp-bridge.*:$PORT" 2>/dev/null || true
echo
echo ">>> starting bridge for '$NAME'"
LOG="$PROFILE/bridge.log"
HOME="$PROFILE" \
OS_MCP_BRIDGE_ADDR="127.0.0.1:$PORT" \
OS_TSEARCH_URL="$CORPUS" \
OS_PORTAL_USER="$NAME" \
OS_MCP_EMAIL_BACKEND="${EMAIL_BACKEND:-auto}" \
  "$ROOT/target/debug/os-mcp-bridge" >"$LOG" 2>&1 &

for _ in $(seq 1 60); do nc -z 127.0.0.1 "$PORT" 2>/dev/null && break; sleep 0.25; done
nc -z 127.0.0.1 "$PORT" 2>/dev/null || { echo "error: bridge did not start — see $LOG" >&2; exit 1; }
echo "    listening (log: $LOG)"

if [[ "$SYNC" == "1" ]]; then
  echo ">>> syncing corpus (this downloads from $CORPUS)"
  printf 'CALL tsearch.sync\n' | nc -w 300 127.0.0.1 "$PORT" | head -2
fi

echo
echo "boot a guest against it:"
echo "    OS_MCP_BRIDGE_ADDR=127.0.0.1:$PORT UTM_VM_NAME=os-$NAME make utm-bridged"
echo "query it directly:"
echo "    BRIDGE_ADDR=127.0.0.1:$PORT ./linux/agent-shell capability"
