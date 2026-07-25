#!/usr/bin/env bash
# Boot the ISO in QEMU and require the serial hello banner + clean debug-exit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ISO="${IMAGE_NAME:-os}.iso"
if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing — run 'make iso' first" >&2
  exit 1
fi

export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

# Python driver: reliable timeout on macOS (no GNU timeout required).
python3 - <<'PY'
import os
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(os.environ.get("PWD", ".")).resolve()
iso = root / f"{os.environ.get('IMAGE_NAME', 'os')}.iso"
serial_path = Path(tempfile.mkstemp(prefix="os-smoke-serial.")[1])
qemu_log = Path(tempfile.mkstemp(prefix="os-smoke-qemu.")[1])

try:
    with qemu_log.open("wb") as logf:
        proc = subprocess.Popen(
            [
                "qemu-system-x86_64",
                "-M", "q35",
                "-m", "512M",
                "-cdrom", str(iso),
                "-boot", "d",
                "-display", "none",
                "-serial", f"file:{serial_path}",
                "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
                "-no-reboot",
            ],
            cwd=root,
            stdout=logf,
            stderr=subprocess.STDOUT,
        )
        try:
            status = proc.wait(timeout=90)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
            print("error: QEMU timed out after 90s", file=sys.stderr)
            print(serial_path.read_bytes(), file=sys.stderr)
            sys.exit(1)

    serial = serial_path.read_bytes()
    # isa-debug-exit: guest writes 0x10 → host status ((0x10 << 1) | 1) = 33
    if status != 33:
        print(f"error: QEMU exited with status {status} (expected 33)", file=sys.stderr)
        print("--- serial ---", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        print("--- qemu ---", file=sys.stderr)
        sys.stderr.buffer.write(qemu_log.read_bytes())
        sys.exit(1)

    if b"os: hello from kernel" not in serial:
        print("error: hello banner not found on serial", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        sys.exit(1)

    print("smoke ok: serial hello + qemu exit 33")
finally:
    serial_path.unlink(missing_ok=True)
    qemu_log.unlink(missing_ok=True)
PY
