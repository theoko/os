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

bridge_alive() {
  [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null
}

wait_for_listener() {
  for _ in $(seq 1 120); do
    listening && return 0
    bridge_alive || return 1
    sleep 0.25
  done
  return 1
}

# "Already up" is only good news if it is running the code we just built.
# `make utm-bridged` rebuilds the binary and then found the old process still
# listening, so bridge changes silently never took effect - the guest kept
# talking to a build from an hour ago.
stale() {
  local pid bin_time proc_time
  [[ -f "$PID_FILE" ]] || return 0
  pid="$(cat "$PID_FILE" 2>/dev/null)" || return 0
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null || return 0
  [[ -x "$BRIDGE_BIN" ]] || return 1
  bin_time="$(stat -f%m "$BRIDGE_BIN" 2>/dev/null || echo 0)"
  proc_time="$(ps -o lstart= -p "$pid" 2>/dev/null | xargs -0 -I{} date -j -f "%a %b %d %T %Y" {} +%s 2>/dev/null || echo 0)"
  [[ "$bin_time" -gt "$proc_time" ]]
}

BRIDGE_BIN="target/debug/os-mcp-bridge"

if listening; then
  if stale; then
    echo "bridge restarting: binary is newer than the running process"
    kill "$(cat "$PID_FILE")" 2>/dev/null || true
    for _ in $(seq 1 20); do
      listening || break
      sleep 0.25
    done
  else
    echo "bridge ok: already up (${CONNECT:-$ADDR})"
    exit 0
  fi
fi

# Do not replace a healthy process while it is still warming its corpus. A
# second invocation of `make utm-bridged` used to kill that process, start a
# competing one, and leave QEMU with Connection refused.
if bridge_alive; then
  echo "bridge warming: waiting for ${CONNECT:-$ADDR}"
  if wait_for_listener; then
    echo "bridge ok: ready (${CONNECT:-$ADDR})"
    exit 0
  fi
  echo "error: existing bridge did not become ready — see $LOG_FILE" >&2
  tail -20 "$LOG_FILE" >&2 || true
  exit 1
fi

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

env_args=(OS_MCP_EMAIL_BACKEND="${EMAIL_BACKEND:-auto}")
if [[ -n "$CONNECT" ]]; then
  env_args+=(OS_MCP_BRIDGE_CONNECT="$CONNECT")
else
  env_args+=(OS_MCP_BRIDGE_ADDR="$ADDR")
fi

# UTM's launch script quits and reopens the app after refreshing its bundle.
# Detach the bridge completely so that host-side launcher lifetime can never
# take the listener down beneath a running guest.
nohup env "${env_args[@]}" "$BRIDGE_BIN" >"$LOG_FILE" 2>&1 < /dev/null &
echo $! >"$PID_FILE"

# Wait for the socket to actually accept, not merely for the process to exist.
# `sleep 0.5` reported success while the port was still closed, and the very
# next make step launched QEMU against it — QEMU refuses to start when its
# chardev cannot connect, so a slow bridge startup read as a boot failure.
if ! wait_for_listener; then
  echo "error: bridge did not begin listening on ${CONNECT:-$ADDR} — see $LOG_FILE" >&2
  tail -20 "$LOG_FILE" >&2 || true
  exit 1
fi
echo "bridge started: ${CONNECT:-$ADDR} (pid $(cat "$PID_FILE"), log $LOG_FILE)"
