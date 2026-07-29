#!/usr/bin/env bash
# Build the live teddyOS ISO from this Mac, through the Debian guest.
#
# linux/iso/build-iso.sh must run on Debian (live-build, debootstrap, loop
# devices). The guest that scripts/teddyos-vm.sh boots is that machine. This
# wrapper is the host half: resolve the commit HERE (the guest tree has no
# .git), copy the tree in, run the build with TEDDYOS_COMMIT set, and pull the
# ISO back into dist/.
#
#   ./scripts/build-linux-iso.sh              # native arch of the guest
#   ./scripts/build-linux-iso.sh --arch amd64 # cross-build on an arm64 guest
#
# Requires: a reachable guest (ssh -p 2222 teddy@localhost) with enough disk.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
TARGET="${TEDDYOS_VM_HOST:-$VM_USER@localhost}"
REMOTE_DIR="${TEDDYOS_ISO_REMOTE:-/home/$VM_USER/os-build}"
REMOTE_OUT="${TEDDYOS_ISO_REMOTE_OUT:-/home/$VM_USER/teddyos-iso}"
LOCAL_OUT="${TEDDYOS_ISO_OUT:-$ROOT/dist}"
ARCH_ARG=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH_ARG=(--arch "$2"); shift 2 ;;
    --out)  LOCAL_OUT="$2"; shift 2 ;;
    --help|-h)
      sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "error: unknown argument $1" >&2; exit 2 ;;
  esac
done

SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
run() { ssh "${SSH_OPTS[@]}" "$TARGET" "$@"; }

command -v rsync >/dev/null 2>&1 || {
  echo "error: rsync not found" >&2; exit 1
}

ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET on port $PORT" >&2
  echo "       boot it first: ./scripts/teddyos-vm.sh --headless" >&2
  exit 1
}

# Host commit is the source of truth. The guest copy has no .git.
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
VERSION="$(cat VERSION 2>/dev/null || echo 0.0.0)"
if [[ "$COMMIT" == "unknown" ]]; then
  echo "error: cannot resolve git commit on the host — refusing to stamp the image" >&2
  exit 1
fi

echo ">>> host commit $COMMIT (teddyOS $VERSION)"
echo ">>> syncing tree -> $TARGET:$REMOTE_DIR"

# Stamp the commit into the tree so even a manual guest re-run of build-iso.sh
# (without re-exporting the env) still gets the right value.
echo "$COMMIT" >"$ROOT/.teddyos-commit"
trap 'rm -f "$ROOT/.teddyos-commit"' EXIT

run "mkdir -p '$REMOTE_DIR' '$REMOTE_OUT'"
# No .git: the guest does not need history, and shipping it would make a stale
# rev-parse look authoritative when TEDDYOS_COMMIT was forgotten.
rsync -az --delete \
  -e "ssh ${SSH_OPTS[*]}" \
  --exclude '.git/' \
  --exclude 'target/' \
  --exclude 'dist/' \
  --exclude '.teddyos-vm/' \
  --exclude '.linux-vm/' \
  --exclude 'edk2-ovmf/' \
  --exclude 'limine/' \
  --exclude '*.iso' \
  --exclude '.claude/' \
  --exclude '__pycache__/' \
  --exclude '.DS_Store' \
  "$ROOT/" "$TARGET:$REMOTE_DIR/"

echo ">>> building ISO on the guest (this is the long step)"
# Export TEDDYOS_COMMIT and point TEDDYOS_ISO_OUT at a known path so we can
# pull the result back without guessing the build stamp in the filename.
BUILD_CMD="./build-iso.sh"
if [[ ${#ARCH_ARG[@]} -gt 0 ]]; then
  BUILD_CMD="./build-iso.sh ${ARCH_ARG[0]} ${ARCH_ARG[1]}"
fi
run "export TEDDYOS_COMMIT='$COMMIT' TEDDYOS_ISO_OUT='$REMOTE_OUT'
     cd '$REMOTE_DIR/linux/iso' && $BUILD_CMD"

echo ">>> fetching ISO(s) into $LOCAL_OUT"
mkdir -p "$LOCAL_OUT"
# Newest image only — the guest keeps prior builds under the same out dir.
newest="$(run "ls -1t '$REMOTE_OUT'/teddyos-*.iso 2>/dev/null | head -1")"
if [[ -z "$newest" ]]; then
  echo "error: no teddyos-*.iso under $REMOTE_OUT on the guest" >&2
  exit 1
fi
rsync -az -e "ssh ${SSH_OPTS[*]}" "$TARGET:$newest" "$LOCAL_OUT/"
local_name="$(basename "$newest")"
echo ">>> $LOCAL_OUT/$local_name"
echo "    commit=$COMMIT  (also in /etc/teddyos-software inside the image)"
