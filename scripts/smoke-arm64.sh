#!/usr/bin/env bash
# Boot the ARM64 ISO under QEMU virt and require the UI to actually render.
#
# The x86 smoke asserts on the serial banner and an `isa-debug-exit` status.
# Neither exists here: `virt` has no debug-exit device, and the PL011 is
# unreachable because Limine's HHDM maps RAM but not device MMIO — writing to
# the UART aborts into a vector table the kernel has not installed yet. So the
# assertion is the framebuffer: screendump it over the QEMU monitor and check
# the OS palette is on screen. That is a stronger claim than a banner anyway —
# it means Limine handed off, the kernel ran, and the compositor drew a frame.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ISO="${ARM64_IMAGE_NAME:-os-arm64}.iso"
if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing — run 'make arm64-iso' first" >&2
  exit 1
fi

FIRMWARE=""
for f in /opt/homebrew/share/qemu/edk2-aarch64-code.fd \
         /usr/local/share/qemu/edk2-aarch64-code.fd \
         /usr/share/qemu/edk2-aarch64-code.fd; do
  [[ -f "$f" ]] && { FIRMWARE="$f"; break; }
done
if [[ -z "$FIRMWARE" ]]; then
  echo "error: edk2-aarch64-code.fd not found — install qemu" >&2
  exit 1
fi

export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="$ISO"
export OS_SMOKE_FIRMWARE="$FIRMWARE"
export OS_SMOKE_BOOT_SECS="${OS_SMOKE_BOOT_SECS:-35}"

python3 <<'PY'
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
firmware = os.environ["OS_SMOKE_FIRMWARE"]
boot_secs = int(os.environ["OS_SMOKE_BOOT_SECS"])

serial_path = Path(tempfile.mkstemp(prefix="os-arm64-serial.")[1])
shot = Path(tempfile.mkstemp(prefix="os-arm64-screen.", suffix=".ppm")[1])
# UEFI wants a writable variable store; a throwaway keeps runs independent.
vars_path = Path(tempfile.mkstemp(prefix="os-arm64-vars.")[1])
vars_path.write_bytes(b"\0" * (64 * 1024 * 1024))

# Port 0 would be ideal but QEMU needs a concrete one; pick a free port first.
probe = socket.socket()
probe.bind(("127.0.0.1", 0))
port = probe.getsockname()[1]
probe.close()

proc = subprocess.Popen(
    [
        "qemu-system-aarch64",
        "-M", "virt",
        "-cpu", "cortex-a72",
        "-m", "512M",
        "-device", "ramfb",
        "-display", "none",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=pflash,format=raw,file={vars_path}",
        "-cdrom", str(iso),
        "-serial", f"file:{serial_path}",
        "-monitor", f"tcp:127.0.0.1:{port},server,nowait",
        "-no-reboot",
    ],
    cwd=root,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.PIPE,
)


def cleanup():
    if proc.poll() is None:
        proc.kill()
    proc.wait()
    for p in (serial_path, vars_path, shot):
        p.unlink(missing_ok=True)


def fail(msg, extra=b""):
    print(f"error: {msg}", file=sys.stderr)
    if extra:
        sys.stderr.buffer.write(extra + b"\n")
    tail = serial_path.read_bytes()[-2000:]
    if tail:
        print("--- serial (firmware/limine) ---", file=sys.stderr)
        sys.stderr.buffer.write(tail + b"\n")
    cleanup()
    sys.exit(1)


# Firmware + Limine + kernel need time before a frame exists.
time.sleep(boot_secs)
if proc.poll() is not None:
    fail(f"qemu exited early with status {proc.returncode}")

try:
    mon = socket.create_connection(("127.0.0.1", port), 5)
    mon.settimeout(10)
    time.sleep(0.5)
    try:
        mon.recv(65536)
    except OSError:
        pass
    mon.send(f"screendump {shot}\n".encode())
    time.sleep(3)
    mon.close()
except OSError as e:
    fail(f"could not reach the qemu monitor: {e}")

if not shot.exists() or shot.stat().st_size == 0:
    fail("screendump produced nothing — no framebuffer was ever created")

raw = shot.read_bytes()
if not raw.startswith(b"P6"):
    fail("screendump is not a PPM")
_, dims, _maxval, body = raw.split(b"\n", 3)
w, h = (int(v) for v in dims.split())

# The OS home/setup screens are a white page with Apple-ish accent and ink.
# Requiring the accent proves real UI drawing, not just a cleared screen.
PAGE = b"\xff\xff\xff"
ACCENT = b"\x00\x71\xe3"
INK = b"\x1d\x1d\x1f"
seen = {}
for y in range(0, h, 4):
    for x in range(0, w, 4):
        i = (y * w + x) * 3
        px = body[i:i + 3]
        seen[px] = seen.get(px, 0) + 1

page = seen.get(PAGE, 0)
accent = seen.get(ACCENT, 0)
ink = seen.get(INK, 0)
if page == 0:
    fail(f"no page-coloured pixels at {w}x{h}; screen is not the OS UI")
if accent == 0 and ink == 0:
    fail(
        f"page is blank at {w}x{h} — cleared, but nothing was drawn on it "
        f"(distinct colours seen: {len(seen)})"
    )

print(
    f"smoke-arm64 ok: aarch64 kernel rendered its UI at {w}x{h} "
    f"(page={page} accent={accent} ink={ink} sampled pixels)"
)
cleanup()
PY
