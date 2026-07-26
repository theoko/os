#!/usr/bin/env bash
# Boot the ARM64 ISO under QEMU virt and require the serial hello banner.
#
# Unlike the x86 smoke there is no `isa-debug-exit` device on `virt`, so the
# guest cannot hand us an exit status. The assertion is therefore the serial
# output alone, and the VM is stopped by timeout rather than by exiting itself.
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

export OS_SMOKE_ISO="$ISO"
export OS_SMOKE_FIRMWARE="$FIRMWARE"
export OS_SMOKE_ROOT="$ROOT"

python3 <<'PY'
import os
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
firmware = os.environ["OS_SMOKE_FIRMWARE"]
serial_path = Path(tempfile.mkstemp(prefix="os-smoke-arm64-serial.")[1])

# UEFI firmware wants a writable variable store; a read-only pflash pair makes
# some edk2 builds refuse to boot. A throwaway copy keeps runs independent.
vars_path = Path(tempfile.mkstemp(prefix="os-smoke-arm64-vars.")[1])
with open(vars_path, "wb") as f:
    f.write(b"\0" * (64 * 1024 * 1024))

proc = subprocess.Popen(
    [
        "qemu-system-aarch64",
        "-M", "virt",
        "-cpu", "cortex-a72",
        "-m", "512M",
        "-display", "none",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=pflash,format=raw,file={vars_path}",
        "-cdrom", str(iso),
        "-serial", f"file:{serial_path}",
        "-no-reboot",
    ],
    cwd=root,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.PIPE,
)

# The guest has no way to power the machine off, so the boot window IS the
# test: give firmware + Limine + kernel time to reach the banner, then stop.
try:
    _, err = proc.communicate(timeout=90)
    qemu_err = err.decode(errors="replace")
except subprocess.TimeoutExpired:
    proc.kill()
    _, err = proc.communicate()
    qemu_err = err.decode(errors="replace")

serial = serial_path.read_bytes()
serial_path.unlink(missing_ok=True)
vars_path.unlink(missing_ok=True)

if b"os: hello from kernel" not in serial:
    print("error: hello banner not found on arm64 serial", file=sys.stderr)
    print("--- serial ---", file=sys.stderr)
    sys.stderr.buffer.write(serial + b"\n")
    if qemu_err.strip():
        print("--- qemu ---", file=sys.stderr)
        print(qemu_err, file=sys.stderr)
    sys.exit(1)

print("smoke-arm64 ok: serial hello from the aarch64 kernel")
PY
