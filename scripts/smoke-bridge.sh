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
# The smoke must exercise this checkout's bridge. After a branch switch or
# rebase, Cargo can otherwise retain an older executable in target/ when the
# checked-out source mtime predates that artifact. Cleaning just this package
# is cheap and makes the wire contract test deterministic.
cargo clean -p os-mcp-bridge >/dev/null
cargo build -p os-mcp-bridge

SERIAL_OUT="$(mktemp "${TMPDIR:-/tmp}/os-bridge-serial.XXXXXX")"
BRIDGE_LOG="$(mktemp "${TMPDIR:-/tmp}/os-bridge-log.XXXXXX")"
SKILLS_DIR="$(mktemp -d "${TMPDIR:-/tmp}/os-bridge-skills.XXXXXX")"
# Isolate personal-data stores so smoke never touches Application Support.
TRANSCRIPT_STORE="$(mktemp "${TMPDIR:-/tmp}/os-smoke-transcripts.XXXXXX.json")"
GRAPH_PATH="$(mktemp "${TMPDIR:-/tmp}/os-smoke-emails.XXXXXX.json")"
WORK_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/os-smoke-workspace.XXXXXX")"
WORK_INDEX="$(mktemp "${TMPDIR:-/tmp}/os-smoke-workspace-ix.XXXXXX.json")"
printf '%s\n' '# Smoke Workspace Alpha' '' 'Prose about smoke workspace alpha for search.' \
  >"$WORK_ROOT/smoke-workspace-alpha.md"
export OS_TRANSCRIPT_STORE="$TRANSCRIPT_STORE"
export OS_GRAPH_PATH="$GRAPH_PATH"
export OS_WORKSPACE_ROOTS="$WORK_ROOT"
export OS_WORKSPACE_INDEX="$WORK_INDEX"
cleanup() {
  if [[ -n "${BRIDGE_PID:-}" ]]; then kill "$BRIDGE_PID" 2>/dev/null || true; fi
  rm -rf "$SKILLS_DIR" "$WORK_ROOT"
  rm -f "$SERIAL_OUT" "$BRIDGE_LOG" "$TRANSCRIPT_STORE" "$GRAPH_PATH" "$WORK_INDEX"
}
trap cleanup EXIT

OS_MCP_BRIDGE_ADDR="$ADDR" OS_MCP_EMAIL_BACKEND=mock \
  OS_SKILLS_USER="$SKILLS_DIR" \
  OS_TRANSCRIPT_STORE="$TRANSCRIPT_STORE" \
  OS_GRAPH_PATH="$GRAPH_PATH" \
  OS_WORKSPACE_ROOTS="$WORK_ROOT" \
  OS_WORKSPACE_INDEX="$WORK_INDEX" \
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
# A cold Rust process can spend several seconds loading its local index on a
# constrained CI runner. Keep the probe bounded, but don't report that normal
# warm-up as a failed bind.
for _ in $(seq 1 100); do
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
export OS_SMOKE_TRANSCRIPT_STORE="$TRANSCRIPT_STORE"
export OS_SMOKE_WORK_ROOT="$WORK_ROOT"

# Host-side wire checks: portals, email, skills, audio, workspace, doc.read —
# each behind its wire bit (no live whisper).
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
    "audio.transcribe",
    "audio.forget",
    "workspace.index",
    "workspace.forget",
    "doc.read",
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

denied_audio = call("CALL audio.transcribe path=/tmp/os-smoke-missing.wav")
if "needs_audio_cap" not in denied_audio:
    print("error: audio.transcribe must require audio=1", file=sys.stderr)
    print(denied_audio, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: audio.transcribe needs_audio_cap ok")

store = os.environ["OS_SMOKE_TRANSCRIPT_STORE"]
with open(store, "w", encoding="utf-8") as f:
    f.write(
        '{"items":[{"source":"/tmp/os-smoke-rec.wav","title":"Smoke Recording Alpha",'
        '"text":"smoke recording alpha transcript words for search",'
        '"seconds":1.0,"words":6}]}'
    )
hit = call("CALL search.query q=Smoke-Recording-Alpha k=3 audio=1")
if "Smoke Recording Alpha" not in hit:
    print("error: search.query audio=1 missed seeded transcript", file=sys.stderr)
    print(hit, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: search.query audio=1 ok")

audio_url = None
for line in hit.splitlines():
    if line.startswith("ROW ") and "audio://" in line:
        for part in line.split("|"):
            if part.startswith("url="):
                audio_url = part[4:]
                break
if not audio_url:
    print("error: audio search row missing audio:// url", file=sys.stderr)
    print(hit, file=sys.stderr)
    sys.exit(1)
denied_audio_doc = call(f"CALL doc.read url={audio_url} lines=8")
if "needs_audio_cap" not in denied_audio_doc:
    print("error: doc.read audio:// must require audio=1", file=sys.stderr)
    print(denied_audio_doc, file=sys.stderr)
    sys.exit(1)
opened_audio = call(f"CALL doc.read url={audio_url} lines=8 audio=1")
if not opened_audio.startswith("OK doc.read") or "smoke recording alpha" not in opened_audio:
    print("error: doc.read audio=1 must open transcript", file=sys.stderr)
    print(opened_audio, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: doc.read audio=1 ok")

forgot_audio = call("CALL audio.forget")
if not forgot_audio.startswith("OK audio.forget"):
    print("error: audio.forget failed", file=sys.stderr)
    print(forgot_audio, file=sys.stderr)
    sys.exit(1)
miss = call("CALL search.query q=Smoke-Recording-Alpha k=3 audio=1")
if "Smoke Recording Alpha" in miss:
    print("error: audio.forget left transcript searchable", file=sys.stderr)
    print(miss, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: audio.forget ok")

denied_ws = call("CALL workspace.index")
if "needs_workspace_cap" not in denied_ws:
    print("error: workspace.index must require files=1", file=sys.stderr)
    print(denied_ws, file=sys.stderr)
    sys.exit(1)
indexed = call("CALL workspace.index files=1")
if not indexed.startswith("OK workspace.index"):
    print("error: workspace.index files=1 must succeed", file=sys.stderr)
    print(indexed, file=sys.stderr)
    sys.exit(1)
ws_hit = call("CALL search.query q=Smoke-Workspace-Alpha k=3 files=1")
if "Smoke Workspace Alpha" not in ws_hit:
    print("error: search.query files=1 missed seeded workspace doc", file=sys.stderr)
    print(ws_hit, file=sys.stderr)
    sys.exit(1)
# Open the file hit via doc.read.
file_url = None
for line in ws_hit.splitlines():
    if line.startswith("ROW ") and "file://" in line:
        # url=file://rel
        for part in line.split("|"):
            if part.startswith("url="):
                file_url = part[4:]
                break
if not file_url:
    print("error: workspace search row missing file:// url", file=sys.stderr)
    print(ws_hit, file=sys.stderr)
    sys.exit(1)
denied_doc = call(f"CALL doc.read url={file_url} lines=8")
if "needs_workspace_cap" not in denied_doc:
    print("error: doc.read file:// must require files=1", file=sys.stderr)
    print(denied_doc, file=sys.stderr)
    sys.exit(1)
opened = call(f"CALL doc.read url={file_url} lines=8 files=1")
if not opened.startswith("OK doc.read") or "Smoke Workspace Alpha" not in opened:
    print("error: doc.read files=1 must open workspace file", file=sys.stderr)
    print(opened, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: workspace.index + doc.read files=1 ok")

forgot_ws = call("CALL workspace.forget")
if not forgot_ws.startswith("OK workspace.forget"):
    print("error: workspace.forget failed", file=sys.stderr)
    print(forgot_ws, file=sys.stderr)
    sys.exit(1)
ws_miss = call("CALL search.query q=Smoke-Workspace-Alpha k=3 files=1")
if "Smoke Workspace Alpha" in ws_miss:
    print("error: workspace.forget left files searchable", file=sys.stderr)
    print(ws_miss, file=sys.stderr)
    sys.exit(1)
print("smoke-bridge: workspace.forget ok")

# Mail open: re-seed the graph (email.forget ran earlier), then open a hit.
reseed = call("CALL email.search q=in:inbox max=2 email=1")
if not reseed.startswith("OK email.search"):
    print("error: could not re-seed mail graph for doc.read", file=sys.stderr)
    print(reseed, file=sys.stderr)
    sys.exit(1)
mail_hit = call("CALL search.query q=Q2-planning k=3 email=1")
mail_url = None
for line in mail_hit.splitlines():
    if line.startswith("ROW ") and "email://" in line:
        for part in line.split("|"):
            if part.startswith("url="):
                mail_url = part[4:]
                break
if mail_url:
    denied_mail_doc = call(f"CALL doc.read url={mail_url} lines=8")
    if "needs_email_cap" not in denied_mail_doc:
        print("error: doc.read email:// must require email=1", file=sys.stderr)
        print(denied_mail_doc, file=sys.stderr)
        sys.exit(1)
mail_doc = call(f"CALL doc.read url={mail_url} lines=8 email=1") if mail_url else ""
if mail_url and (
    not mail_doc.startswith("OK doc.read") or "Q2 planning" not in mail_doc
):
    print("error: doc.read email=1 must show mail snippet", file=sys.stderr)
    print(mail_doc, file=sys.stderr)
    sys.exit(1)
if mail_url:
    print("smoke-bridge: doc.read email=1 ok")
else:
    print("smoke-bridge: doc.read email=1 skipped (no email:// hit)")
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
print("smoke-bridge ok: hello + mcp email + portals + audio + files")
PY
