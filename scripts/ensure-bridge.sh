#!/usr/bin/env bash
# Ensure the host MCP bridge is available.
#
# Modes:
#   OS_MCP_BRIDGE_ADDR=127.0.0.1:7420     — listen TCP (QEMU / smoke-bridge)
#   OS_MCP_BRIDGE_ADDR=unix:/path.sock   — listen unix
#   OS_MCP_BRIDGE_CONNECT=unix:/path     — dial QEMU's serial unix server (UTM)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
CONNECT="${OS_MCP_BRIDGE_CONNECT:-}"
PID_FILE="${ROOT}/.bridge.pid"
LOG_FILE="${ROOT}/.bridge.log"
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

listening() {
  if [[ -n "$CONNECT" ]]; then
    # Connect mode: "ok" means our bridge process is alive.
    [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null
    return
  fi
  if [[ "$ADDR" == unix:* ]]; then
    local sock="${ADDR#unix:}"
    [[ -S "$sock" ]]
  else
    local host="${ADDR%:*}"
    local port="${ADDR##*:}"
    if command -v nc >/dev/null 2>&1; then
      nc -z "$host" "$port" >/dev/null 2>&1
    else
      lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1
    fi
  fi
}

if listening; then
  echo "bridge ok: already up (${CONNECT:-$ADDR})"
  exit 0
fi

BRIDGE_BIN="target/debug/os-mcp-bridge"
if [[ ! -x "$BRIDGE_BIN" ]]; then
  cargo build -p os-mcp-bridge
fi

if [[ -f "$PID_FILE" ]]; then
  old="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [[ -n "$old" ]]; then
    kill "$old" 2>/dev/null || true
    sleep 0.3
  fi
  rm -f "$PID_FILE"
fi

if [[ -z "$CONNECT" && "$ADDR" == unix:* ]]; then
  sock="${ADDR#unix:}"
  mkdir -p "$(dirname "$sock")"
  rm -f "$sock"
fi

env_args=(OS_MCP_EMAIL_BACKEND="${EMAIL_BACKEND:-mock}")
if [[ -n "$CONNECT" ]]; then
  env_args+=(OS_MCP_BRIDGE_CONNECT="$CONNECT")
else
  env_args+=(OS_MCP_BRIDGE_ADDR="$ADDR")
fi

env "${env_args[@]}" "$BRIDGE_BIN" >"$LOG_FILE" 2>&1 &
echo $! >"$PID_FILE"
sleep 0.5

if ! kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
  echo "error: bridge failed to start — see $LOG_FILE" >&2
  tail -20 "$LOG_FILE" >&2 || true
  exit 1
fi
echo "bridge started: ${CONNECT:-$ADDR} (pid $(cat "$PID_FILE"), log $LOG_FILE)"
