#!/usr/bin/env bash
# Ensure the host MCP bridge is available.
#
# Modes:
#   OS_MCP_BRIDGE_ADDR=127.0.0.1:7420     — listen TCP (QEMU / smoke-bridge)
#   OS_MCP_BRIDGE_ADDR=unix:/path.sock   — listen unix
#   OS_MCP_BRIDGE_CONNECT=tcp:127.0.0.1:7420 — dial UTM COM2 TcpServer (retries)
#   OS_MCP_BRIDGE_CONNECT=unix:/path     — dial QEMU unix serial server
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
CONNECT="${OS_MCP_BRIDGE_CONNECT:-}"
PID_FILE="${ROOT}/.bridge.pid"
LOG_FILE="${ROOT}/.bridge.log"
WAIT_SECS="${OS_MCP_BRIDGE_WAIT_SECS:-60}"
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

listening() {
  if [[ -n "$CONNECT" ]]; then
    # Connect mode: process alive means it is (re)dialing — there is no listen port.
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
      nc -z "$host" "$port" >/dev/null 2>&1 && return 0
    fi
    if command -v python3 >/dev/null 2>&1; then
      python3 -c "import socket; s=socket.create_connection(('${host}', int('${port}')), 1.0); s.close()" >/dev/null 2>&1 && return 0
    fi
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1
  fi
}

bridge_alive() {
  [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null
}

fail_start() {
  echo "error: bridge failed to start — see $LOG_FILE" >&2
  tail -40 "$LOG_FILE" >&2 || true
  exit 1
}

# Free a TCP port so UTM TcpServer (or a listen-mode bridge) can bind it.
free_tcp_port() {
  local port="$1"
  if ! command -v lsof >/dev/null 2>&1; then
    return 0
  fi
  local pids
  pids="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)"
  if [[ -z "$pids" ]]; then
    return 0
  fi
  echo "bridge: freeing TCP :$port (pids: $pids)" >&2
  # shellcheck disable=SC2086
  kill $pids 2>/dev/null || true
  sleep 0.3
  # shellcheck disable=SC2086
  kill -9 $pids 2>/dev/null || true
  sleep 0.2
}

if [[ -f "$PID_FILE" ]] && ! bridge_alive; then
  rm -f "$PID_FILE"
fi

# Dial-mode restart: always replace so we are not stuck on a stale listen-mode
# process holding :7420 (blocks UTM TcpServer).
if [[ -n "$CONNECT" ]]; then
  if [[ "$CONNECT" == tcp:* || "$CONNECT" =~ ^[0-9.]+:[0-9]+$ ]]; then
    port="${CONNECT##*:}"
    free_tcp_port "$port"
  fi
  if bridge_alive; then
    kill "$(cat "$PID_FILE")" 2>/dev/null || true
    sleep 0.2
    rm -f "$PID_FILE"
  fi
elif listening; then
  echo "bridge ok: already up ($ADDR)"
  exit 0
fi

if [[ -z "$CONNECT" ]] && bridge_alive && ! listening; then
  echo "bridge: pid $(cat "$PID_FILE") alive but not listening — restarting" >&2
  kill "$(cat "$PID_FILE")" 2>/dev/null || true
  sleep 0.3
  rm -f "$PID_FILE"
fi

BRIDGE_BIN="target/debug/os-mcp-bridge"
if [[ ! -x "$BRIDGE_BIN" ]]; then
  cargo build -p os-mcp-bridge
fi

if [[ -f "$PID_FILE" ]]; then
  old="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [[ -n "$old" ]]; then
    kill "$old" 2>/dev/null || true
    for _ in $(seq 1 20); do
      bridge_alive || break
      sleep 0.1
    done
    kill -9 "$old" 2>/dev/null || true
    sleep 0.2
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

: >"$LOG_FILE"
env "${env_args[@]}" "$BRIDGE_BIN" >>"$LOG_FILE" 2>&1 &
echo $! >"$PID_FILE"

deadline=$((SECONDS + WAIT_SECS))
while (( SECONDS < deadline )); do
  if ! bridge_alive; then
    fail_start
  fi
  if listening; then
    echo "bridge started: ${CONNECT:-$ADDR} (pid $(cat "$PID_FILE"), log $LOG_FILE)"
    exit 0
  fi
  sleep 0.25
done

echo "error: bridge pid alive but ${CONNECT:-$ADDR} not ready after ${WAIT_SECS}s — see $LOG_FILE" >&2
tail -40 "$LOG_FILE" >&2 || true
exit 1
