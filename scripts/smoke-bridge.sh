#!/usr/bin/env bash
# Boot with COM2 as TCP server; host bridge dials in. Require "mcp: email connected".
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=rust-path.sh
source "$ROOT/scripts/rust-path.sh"

ISO="${IMAGE_NAME:-os}.iso"
ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"

if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing" >&2
  exit 1
fi

OS_MCP_BRIDGE_CONNECT="tcp:$ADDR" ./scripts/ensure-bridge.sh
BRIDGE_PID="$(cat .bridge.pid)"

SERIAL_OUT="$(mktemp "${TMPDIR:-/tmp}/os-bridge-serial.XXXXXX")"
cleanup() {
  if [[ -n "${BRIDGE_PID:-}" ]]; then kill "$BRIDGE_PID" 2>/dev/null || true; fi
  rm -f "$SERIAL_OUT" .bridge.pid
}
trap cleanup EXIT

export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="$ISO"
export OS_SMOKE_ADDR="$ADDR"
export OS_SMOKE_SERIAL="$SERIAL_OUT"
python3 "$ROOT/scripts/smoke_common.py" bridge
