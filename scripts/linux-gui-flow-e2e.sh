#!/usr/bin/env bash
# GUI flow e2e for teddyOS Linux desktop (live guest).
#
# Drives the real GTK apps with xdotool under GDK_BACKEND=x11, and asserts via:
#   - app logs (/var/log/teddyos/app/*.log)
#   - process / window presence
#   - AT-SPI names when exposed
#
#   ./scripts/linux-gui-flow-e2e.sh
#   TEDDYOS_VM_HOST=192.168.64.2 ./scripts/linux-gui-flow-e2e.sh
set -euo pipefail
# Ensure pipeline failures from remote driver surface (tee must not mask them).
set -o pipefail

HOST="${TEDDYOS_VM_HOST:-192.168.64.2}"
USER="${TEDDYOS_VM_USER:-teddy}"
PORT="${TEDDYOS_VM_SSH_PORT:-22}"
TARGET="${USER}@${HOST}"
SSH=(ssh -p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
     -o LogLevel=ERROR -o ConnectTimeout=8 "$TARGET")

OUT_DIR="${GUI_E2E_OUT:-/tmp/teddyos-gui-e2e}"
mkdir -p "$OUT_DIR"

echo "=== teddyOS GUI flow e2e -> $TARGET ==="
echo "evidence: $OUT_DIR"
echo

# Upload and run the remote driver
"${SSH[@]}" 'mkdir -p /tmp/teddyos-gui-e2e'
scp -o StrictHostKeyChecking=no -o LogLevel=ERROR -P "$PORT" \
  scripts/linux-gui-flow-driver.py "$TARGET:/tmp/teddyos-gui-e2e/driver.py"

set +e
"${SSH[@]}" bash -s <<REMOTE
set -euo pipefail
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
export HOME=/home/teddy
export XAUTHORITY=\$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
export PATH=/usr/local/bin:/usr/bin:/bin
export GDK_BACKEND=x11
export GUI_E2E_EVIDENCE=/tmp/teddyos-gui-e2e/evidence
mkdir -p "\$GUI_E2E_EVIDENCE"
# ensure xdotool
command -v xdotool >/dev/null || { echo "FATAL: xdotool missing"; exit 2; }
python3 /tmp/teddyos-gui-e2e/driver.py
REMOTE
RC=$?
set -e

# Pull evidence
scp -o StrictHostKeyChecking=no -o LogLevel=ERROR -P "$PORT" -r \
  "$TARGET:/tmp/teddyos-gui-e2e/evidence/." "$OUT_DIR/" 2>/dev/null || true

echo
if [[ "$RC" -eq 0 ]]; then
  echo "RESULT: PASS (evidence in $OUT_DIR)"
else
  echo "RESULT: FAIL (exit $RC, evidence in $OUT_DIR)"
fi
exit "$RC"
