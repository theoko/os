#!/usr/bin/env bash
# Boot the ISO in QEMU and require the serial hello banner + clean debug-exit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="${IMAGE_NAME:-os}.iso"
python3 "$ROOT/scripts/smoke_common.py" serial
