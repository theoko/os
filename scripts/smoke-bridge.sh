#!/usr/bin/env bash
# Boot with COM2 as TCP server; host bridge dials in. Require "mcp: email connected".
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ISO="${IMAGE_NAME:-os}.iso"
ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

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

python3 <<'PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(os.environ["OS_SMOKE_ROOT"]) / "scripts"))
import smoke_common as sc

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
addr = os.environ["OS_SMOKE_ADDR"]
serial_path = Path(os.environ["OS_SMOKE_SERIAL"])
serial_path.write_bytes(b"")

# Guest listens; wait for the dialing bridge before the guest runs (early PING).
argv = sc.qemu_argv(iso, serial_path, com2=addr)
status, serial = sc.run_qemu(argv, cwd=root, serial_path=serial_path, timeout=120)
sc.check_smoke(status, serial, need_mcp=True)
print("smoke-bridge ok: hello + mcp email connected")
PY
