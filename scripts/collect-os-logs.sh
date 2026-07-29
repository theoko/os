#!/usr/bin/env bash
# Pull teddyOS logs onto this Mac so a broken guest is still debuggable.
#
# The guest keeps its own trail under /var/log/teddyos (journal snapshots +
# app files). When the guest is the thing that failed, you need those files
# HERE. This script:
#
#   1. Asks the guest to take a fresh snapshot (if ssh works)
#   2. rsyncs /var/log/teddyos into ./logs/os/<stamp>/
#   3. Grabs freestanding-kernel serial crumbs that live on the host
#
#   ./scripts/collect-os-logs.sh
#   ./scripts/collect-os-logs.sh --host 192.168.64.2
#   ./scripts/collect-os-logs.sh --out /tmp/os-logs
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT="${TEDDYOS_VM_SSH_PORT:-22}"
HOST="${TEDDYOS_VM_HOST:-}"
USER_NAME="${TEDDYOS_VM_USER:-teddy}"
OUT=""
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host) HOST="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    --user) USER_NAME="$2"; shift 2 ;;
    --out)  OUT="$2"; shift 2 ;;
    --help|-h)
      sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "error: unknown argument $1" >&2; exit 2 ;;
  esac
done

# Default guest: the running UTM shared-net address if nothing was set.
if [[ -z "$HOST" ]]; then
  if [[ -r /var/db/dhcpd_leases ]]; then
    HOST="$(awk -F= '
      $1=="name" && $2 ~ /teddyos/ { want=1 }
      want && $1=="ip_address" { gsub(/[{} ]/,"",$2); print $2; exit }
    ' /var/db/dhcpd_leases 2>/dev/null | head -1)"
  fi
  HOST="${HOST:-192.168.64.2}"
fi

OUT="${OUT:-$ROOT/logs/os/$STAMP}"
mkdir -p "$OUT"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
TARGET="$USER_NAME@$HOST"

echo ">>> collecting into $OUT"
echo "    guest $TARGET"

if ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null; then
  echo ">>> check diagnostics.share on guest"
  ssh "${SSH_OPTS[@]}" "$TARGET" \
    'python3 -c "import sys; sys.path.insert(0,\"/usr/lib/teddyos\"); import caps; print(\"diagnostics.share\", caps.diagnostics_share_allowed())"' \
    | tee "$OUT/guest-consent.txt" || true

  echo ">>> snapshot on guest (respects diagnostics.share; use FORCE=1 to override)"
  # Prefer rootless path if sudo is not available; collect script falls back.
  # FORCE=1 is for operators debugging a machine they own — it is not the
  # product default and is never set by setup.
  FORCE_FLAG=""
  if [[ "${FORCE:-0}" == "1" ]]; then
    FORCE_FLAG="--force"
    echo "    FORCE=1: collecting even without consent" | tee -a "$OUT/guest-consent.txt"
  fi
  ssh "${SSH_OPTS[@]}" "$TARGET" \
    "sudo -n teddyos-log-collect $FORCE_FLAG 2>/dev/null || teddyos-log-collect $FORCE_FLAG 2>/dev/null || true" \
    | tee "$OUT/guest-collect.txt" || true

  echo ">>> rsync /var/log/teddyos"
  rsync -az -e "ssh ${SSH_OPTS[*]}" \
    --ignore-errors \
    "$TARGET:/var/log/teddyos/" "$OUT/guest-var-log-teddyos/" 2>/dev/null || true

  echo ">>> rsync user state logs"
  rsync -az -e "ssh ${SSH_OPTS[*]}" \
    --ignore-errors \
    "$TARGET:$USER_NAME/.local/state/teddyos/" "$OUT/guest-user-state/" 2>/dev/null || true

  # One-shot journal export in case the snapshot binary is not installed yet.
  echo ">>> journal export (this boot)"
  ssh "${SSH_OPTS[@]}" "$TARGET" \
    'journalctl -b --no-pager -o short-iso 2>/dev/null | tail -n 5000' \
    >"$OUT/guest-journal-boot.txt" 2>/dev/null || true
  ssh "${SSH_OPTS[@]}" "$TARGET" \
    'journalctl -t "teddyos-*" --no-pager -o short-iso -n 2000 2>/dev/null' \
    >"$OUT/guest-journal-teddyos.txt" 2>/dev/null || true
else
  echo ">>> guest not reachable at $TARGET — host-only crumbs" | tee "$OUT/guest-unreachable.txt"
fi

# Host-side freestanding kernel / VM crumbs (serial files, bridge logs, …).
echo ">>> host crumbs"
mkdir -p "$OUT/host"
for f in \
  /tmp/teddyos-arm64.log \
  /tmp/teddyos-*-uart.log \
  /tmp/os-smoke-serial.* \
  /tmp/os-arm64-serial.* \
  "$ROOT"/.bridge.log \
  "$ROOT"/utm-diag-serial.out \
  "$ROOT"/.refresh.log \
  "$ROOT"/.check-published.log
do
  # shellcheck disable=SC2086
  for hit in $f; do
    [[ -e "$hit" ]] || continue
    cp -a "$hit" "$OUT/host/" 2>/dev/null || true
  done
done

# UTM serial if attached via utmctl
if command -v utmctl >/dev/null 2>&1; then
  utmctl list >"$OUT/host/utmctl-list.txt" 2>/dev/null || true
fi

# Point of origin for the tree that produced this guest.
{
  echo "collected_at=$STAMP"
  echo "host=$(hostname)"
  echo "repo=$ROOT"
  echo "commit=$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
  echo "guest=$TARGET"
} >"$OUT/META.txt"

ln -sfn "$STAMP" "$(dirname "$OUT")/latest" 2>/dev/null || true

echo ">>> done"
du -sh "$OUT" 2>/dev/null || true
echo "    $OUT"
