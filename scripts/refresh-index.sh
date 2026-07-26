#!/usr/bin/env bash
# Refresh what search knows: re-index workspace files, re-sync the portal corpus.
#
# Consent rule: the portal corpus is only re-synced if it has ALREADY been
# synced once. portal.sync is granted inside the OS, and a host-side timer must
# not be a way to start talking to the network on the user's behalf. Refreshing
# something they already opted into is fine; opting them in is not.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
HOST="${ADDR%:*}"
PORT="${ADDR##*:}"
BIN="$ROOT/target/debug/os-mcp-bridge"
CORPUS="$HOME/Library/Application Support/os/knowledge/teddy.json"
STARTED=""

log() { printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

call() {
  # One request, one response. nc exits when the bridge closes or we time out.
  printf '%s\n' "$1" | nc -w "${2:-120}" "$HOST" "$PORT" 2>/dev/null | head -1
}

if [[ ! -x "$BIN" ]]; then
  log "bridge binary missing — run: make bridge"
  exit 1
fi

# Reuse a running bridge rather than fighting it for the port.
if ! nc -z "$HOST" "$PORT" 2>/dev/null; then
  log "starting bridge on $ADDR"
  OS_MCP_BRIDGE_ADDR="$ADDR" OS_MCP_EMAIL_BACKEND="${EMAIL_BACKEND:-mock}" \
    "$BIN" >>"$ROOT/.refresh-bridge.log" 2>&1 &
  STARTED=$!
  for _ in $(seq 1 40); do
    nc -z "$HOST" "$PORT" 2>/dev/null && break
    sleep 0.25
  done
else
  log "reusing bridge already on $ADDR"
fi

log "workspace: $(call 'CALL workspace.index files=1' 300)"

if [[ -f "$CORPUS" ]]; then
  log "portal: $(call 'CALL tsearch.sync portal=1' 600)"
else
  log "portal: skipped — never synced, so the grant was never given in the OS"
fi

if [[ -n "$STARTED" ]]; then
  log "stopping bridge we started (pid $STARTED)"
  kill "$STARTED" 2>/dev/null || true
fi
log "done"
