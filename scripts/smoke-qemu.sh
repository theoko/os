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
export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="$ISO"

# Python driver: reliable timeout on macOS (no GNU timeout required).
python3 - <<'PY'
import os
import sys
import tempfile
from pathlib import Path

# Heredoc has no __file__; import from the repo scripts dir.
sys.path.insert(0, str(Path(os.environ["OS_SMOKE_ROOT"]) / "scripts"))
import smoke_common as sc

root = Path(os.environ["OS_SMOKE_ROOT"])
iso = root / os.environ["OS_SMOKE_ISO"]
serial_path = Path(tempfile.mkstemp(prefix="os-smoke-serial.")[1])
qemu_log = Path(tempfile.mkstemp(prefix="os-smoke-qemu.")[1])

try:
    argv = sc.qemu_argv(iso, serial_path)
    status, serial = sc.run_qemu(
        argv, cwd=root, serial_path=serial_path, timeout=90, log_path=qemu_log
    )
    sc.check_smoke(status, serial, qemu_log=qemu_log)
    print("smoke ok: serial hello + qemu exit 33")
finally:
    serial_path.unlink(missing_ok=True)
    qemu_log.unlink(missing_ok=True)
PY
