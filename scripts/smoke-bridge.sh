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

chmod +x scripts/ensure-bridge.sh
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
import os, subprocess, sys
from pathlib import Path

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
addr = os.environ["OS_SMOKE_ADDR"]
serial_path = Path(os.environ["OS_SMOKE_SERIAL"])
serial_path.write_bytes(b"")

# Guest listens; bridge (already dialing) connects. server,nowait = boot without peer.
proc = subprocess.Popen(
    [
        "qemu-system-x86_64",
        "-M", "q35",
        "-m", "512M",
        "-cdrom", str(iso),
        "-boot", "d",
        "-display", "none",
        "-serial", f"file:{serial_path}",
        "-serial", f"tcp:{addr},server,nowait",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-no-reboot",
    ],
    cwd=root,
)
try:
    status = proc.wait(timeout=120)
except subprocess.TimeoutExpired:
    proc.kill()
    proc.wait()
    print("error: QEMU timed out", file=sys.stderr)
    sys.exit(1)

serial = serial_path.read_bytes()
if status != 33:
    print(f"error: QEMU status {status} (expected 33)", file=sys.stderr)
    sys.stderr.buffer.write(serial + b"\n")
    sys.exit(1)
if b"os: hello from kernel" not in serial:
    print("error: missing hello banner", file=sys.stderr)
    sys.stderr.buffer.write(serial + b"\n")
    sys.exit(1)
if b"mcp: email connected" not in serial:
    print("error: MCP bridge not connected", file=sys.stderr)
    sys.stderr.buffer.write(serial + b"\n")
    sys.exit(1)
print("smoke-bridge ok: hello + mcp email connected")
PY
