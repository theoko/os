#!/usr/bin/env bash
# Boot the ISO in QEMU and require the serial hello banner + clean debug-exit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=rust-path.sh
source "$ROOT/scripts/rust-path.sh"

ISO="${IMAGE_NAME:-os}.iso"
if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing — run 'make iso' first" >&2
  exit 1
fi

export OS_SMOKE_ROOT="$ROOT"
export OS_SMOKE_ISO="$ISO"
python3 "$ROOT/scripts/smoke_common.py" serial
