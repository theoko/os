#!/usr/bin/env bash
# Start the host MCP bridge in dial mode (guest listens on COM2; we connect).
#
#   OS_MCP_BRIDGE_CONNECT=tcp:127.0.0.1:7420 ./scripts/ensure-bridge.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=rust-path.sh
source "$ROOT/scripts/rust-path.sh"

CONNECT="${OS_MCP_BRIDGE_CONNECT:-tcp:127.0.0.1:7420}"
ADDR="${CONNECT#tcp:}"
PORT="${ADDR##*:}"
PID_FILE="${ROOT}/.bridge.pid"
LOG_FILE="${ROOT}/.bridge.log"

bridge_alive() {
  [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null
}

# Drop anything holding the guest listen port (stale listen-mode bridge, etc.).
if command -v lsof >/dev/null 2>&1; then
  pids="$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    echo "bridge: freeing :$PORT (pids: $pids)" >&2
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null || true
    sleep 0.2
    # shellcheck disable=SC2086
    kill -9 $pids 2>/dev/null || true
  fi
fi

if bridge_alive; then
  kill "$(cat "$PID_FILE")" 2>/dev/null || true
  sleep 0.2
fi
rm -f "$PID_FILE"

BRIDGE_BIN="target/debug/os-mcp-bridge"
if [[ ! -x "$BRIDGE_BIN" ]]; then
  cargo build -p os-mcp-bridge
fi

: >"$LOG_FILE"
OS_MCP_EMAIL_BACKEND="${EMAIL_BACKEND:-mock}" \
  OS_MCP_BRIDGE_CONNECT="$CONNECT" \
  "$BRIDGE_BIN" >>"$LOG_FILE" 2>&1 &
echo $! >"$PID_FILE"
sleep 0.2

if ! bridge_alive; then
  echo "error: bridge failed to start — see $LOG_FILE" >&2
  tail -40 "$LOG_FILE" >&2 || true
  exit 1
fi
echo "bridge dialing: $CONNECT (pid $(cat "$PID_FILE"), log $LOG_FILE)"
