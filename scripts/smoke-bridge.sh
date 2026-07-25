#!/usr/bin/env bash
# Boot with COM2 wired to the host MCP bridge; require "mcp: email connected".
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ISO="${IMAGE_NAME:-os}.iso"
# A fixed default port lets another local bridge satisfy the probe while this
# script's bridge failed to bind. Pick an isolated loopback port unless the
# caller deliberately supplied one.
ADDR="${OS_MCP_BRIDGE_ADDR:-}"
if [[ -z "$ADDR" ]]; then
  PORT="$(python3 - <<'PY'
import socket

s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
  ADDR="127.0.0.1:${PORT}"
fi
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing" >&2
  exit 1
fi

BRIDGE_BIN="target/debug/os-mcp-bridge"
if [[ ! -x "$BRIDGE_BIN" ]]; then
  cargo build -p os-mcp-bridge
fi

SERIAL_OUT="$(mktemp "${TMPDIR:-/tmp}/os-bridge-serial.XXXXXX")"
BRIDGE_LOG="$(mktemp "${TMPDIR:-/tmp}/os-bridge-log.XXXXXX")"
SKILLS_DIR="$(mktemp -d "${TMPDIR:-/tmp}/os-bridge-skills.XXXXXX")"
cleanup() {
  if [[ -n "${BRIDGE_PID:-}" ]]; then kill "$BRIDGE_PID" 2>/dev/null || true; fi
  rm -rf "$SKILLS_DIR"
  rm -f "$SERIAL_OUT" "$BRIDGE_LOG"
}
trap cleanup EXIT

OS_MCP_BRIDGE_ADDR="$ADDR" OS_MCP_EMAIL_BACKEND=mock OS_SKILLS_USER="$SKILLS_DIR" \
  "$BRIDGE_BIN" >"$BRIDGE_LOG" 2>&1 &
BRIDGE_PID=$!

# Wait until the bridge is actually accepting connections (a fixed sleep races
# a slow bind, and QEMU's tcp: client chardev does not retry). Also fail fast
# if the bridge died on startup, e.g. port already in use.
BRIDGE_HOST="${ADDR%:*}"
BRIDGE_PORT="${ADDR##*:}"
# Prefer a real TCP connect over `nc -z`: many CI images ship neither
# netcat-openbsd nor traditional nc, and bash /dev/tcp is not portable.
bridge_probe() {
  python3 - "$BRIDGE_HOST" "$BRIDGE_PORT" <<'PY'
import socket, sys
host, port = sys.argv[1], int(sys.argv[2])
s = socket.socket()
s.settimeout(0.2)
try:
    s.connect((host, port))
except OSError:
    sys.exit(1)
finally:
    s.close()
PY
}
bridge_up=0
for _ in $(seq 1 50); do
  if ! kill -0 "$BRIDGE_PID" 2>/dev/null; then
    echo "error: bridge exited during startup" >&2
    cat "$BRIDGE_LOG" >&2
    exit 1
  fi
  if bridge_probe; then
    bridge_up=1
    break
  fi
  sleep 0.1
done
if [[ "$bridge_up" != 1 ]]; then
  echo "error: bridge never listened on $ADDR" >&2
  cat "$BRIDGE_LOG" >&2
  exit 1
fi

export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="$ISO"
export OS_SMOKE_ADDR="$ADDR"
export OS_SMOKE_SERIAL="$SERIAL_OUT"

# Host-side wire checks: teddy/market portals, email.search/email.forget,
# skills.save — each behind its wire bit.
python3 <<'PY'
import os, socket, sys

addr = os.environ["OS_SMOKE_ADDR"]
host, port_s = addr.rsplit(":", 1)
port = int(port_s)

def call(line: str, timeout: float = 8.0) -> str:
    s = socket.create_connection((host, port), timeout)
    s.settimeout(timeout)
    try:
        s.sendall((line + "\n").encode())
        chunks = []
        while True:
            try:
                b = s.recv(4096)
            except socket.timeout:
                break
            if not b:
                break
            chunks.append(b)
            text = b"".join(chunks).decode("utf-8", "replace")
            if "\nEND\n" in text or text.startswith("ERR ") or text.startswith("OK tools="):
                # LIST has no END; OK tools= is enough. Portal ERR is one line.
                if text.startswith("OK tools=") or text.startswith("ERR "):
                    break
                if "\nEND\n" in text or text.rstrip().endswith("END"):
                    break
        return b"".join(chunks).decode("utf-8", "replace")
    finally:
        s.close()

def require_listed(listing: str, *tools: str) -> None:
    for tool in tools:
        if tool not in listing:
            print(f"error: LIST missing {tool}", file=sys.stderr)
            print(listing, file=sys.stderr)
            sys.exit(1)

def require_portal_cap(tool: str) -> None:
    denied = call(f"CALL {tool}")
    if "needs_portal_cap" not in denied:
        print(f"error: {tool} must require portal=1", file=sys.stderr)
        print(denied, file=sys.stderr)
        sys.exit(1)

def try_live(tool: str) -> None:
    allowed = call(f"CALL {tool} portal=1", timeout=25.0)
    if allowed.startswith(f"OK {tool}"):
        print(f"smoke-bridge: {tool} portal=1 ok")
    elif "needs_portal_cap" in allowed:
        print(f"error: {tool} portal=1 still denied", file=sys.stderr)
        print(allowed, file=sys.stderr)
        sys.exit(1)
    else:
        print(f"smoke-bridge: {tool} live call skipped ({allowed.splitlines()[:1]})")

listing = call("LIST")
require_listed(
    listing,
    "tsearch.sync",
    "teddy.health",
    "teddy.fear_greed",
    "teddy.gex",
    "market.health",
    "market.fear_greed",
    "portal.forget",
    "email.forget",
    "skills.save",
)

require_portal_cap("teddy.health")
require_portal_cap("market.health")
try_live("teddy.health")
try_live("market.health")

denied_save = call("CALL skills.save name=smoke-denied desc=nope")
if "needs_skills_cap" not in denied_save:
    print("error: skills.save must require skills=1", file=sys.stderr)
    print(denied_save, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: skills.save needs_skills_cap ok")

allowed_save = call(
    "CALL skills.save name=smoke-guest-starter desc=from-smoke skills=1"
)
if not allowed_save.startswith("OK skills.save"):
    print("error: skills.save skills=1 must succeed", file=sys.stderr)
    print(allowed_save, file=sys.stderr)
    sys.exit(1)
listed = call("CALL skills.list")
if "name=smoke-guest-starter" not in listed or "src=saved" not in listed:
    print("error: saved skill missing from skills.list", file=sys.stderr)
    print(listed, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: skills.save skills=1 ok")

forgotten = call("CALL portal.forget")
if not forgotten.startswith("OK portal.forget"):
    print("error: portal.forget failed", file=sys.stderr)
    print(forgotten, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: portal.forget ok")

denied_mail = call("CALL email.search q=in:inbox max=2")
if "needs_email_cap" not in denied_mail:
    print("error: email.search must require email=1", file=sys.stderr)
    print(denied_mail, file=sys.stderr)
    sys.exit(1)
allowed_mail = call("CALL email.search q=in:inbox max=2 email=1")
if not allowed_mail.startswith("OK email.search"):
    print("error: email.search email=1 must succeed", file=sys.stderr)
    print(allowed_mail, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: email.search email=1 ok")

forgot_mail = call("CALL email.forget")
if not forgot_mail.startswith("OK email.forget"):
    print("error: email.forget failed", file=sys.stderr)
    print(forgot_mail, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: email.forget ok")
PY

python3 <<'PY'
import os, subprocess, sys
from pathlib import Path

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
addr = os.environ["OS_SMOKE_ADDR"]
serial_path = Path(os.environ["OS_SMOKE_SERIAL"])
serial_path.write_bytes(b"")

proc = subprocess.Popen(
    [
        "qemu-system-x86_64",
        "-M", "q35",
        "-m", "512M",
        "-cdrom", str(iso),
        "-boot", "d",
        "-display", "none",
        "-serial", f"file:{serial_path}",
        "-serial", f"tcp:{addr}",
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
print("smoke-bridge ok: hello + mcp email + teddy/market + email.forget")
PY
