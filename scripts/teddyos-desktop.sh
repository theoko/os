#!/usr/bin/env bash
# One command for the product that matters: Linux teddyOS in UTM.
#
# Freestanding kernel ISOs (os.iso / os-arm64.iso, "hello" setup) are not this
# path. They remain under make kernel-iso / make kernel-utm for CI and lab work.
#
#   make desktop          # open/start the Linux guest in UTM
#   make linux-init       # first-time: download Debian + cloud-init disk
#   make linux-provision  # install GNOME + Chromium over ssh (headless QEMU)
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WORK="${TEDDYOS_VM_WORK:-$ROOT/.teddyos-vm}"
DISK="$WORK/disk.qcow2"
START="${TEDDYOS_UTM_START:-1}"
MODE="desktop"

usage() {
  cat <<'USAGE'
usage: scripts/teddyos-desktop.sh [--init|--provision|--desktop] [--no-start]

  --desktop    (default) refresh UTM "teddyos" from the Linux disk and start it
  --init       create the aarch64 Debian disk (download + first boot for cloud-init)
  --provision  install GNOME desktop into a running headless guest (ssh :2222)
  --no-start   with --desktop, configure UTM but do not power on

The freestanding kernel ("hello") is deliberately not launched from here.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --init) MODE=init; shift ;;
    --provision) MODE=provision; shift ;;
    --desktop) MODE=desktop; shift ;;
    --no-start) START=0; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "error: unknown argument $1 (try --help)" >&2; exit 2 ;;
  esac
done

chmod +x \
  "$ROOT/scripts/teddyos-vm.sh" \
  "$ROOT/scripts/teddyos-utm.sh" \
  "$ROOT/scripts/teddyos-provision.sh" 2>/dev/null || true

linux_init() {
  if [[ -f "$DISK" ]]; then
    echo ">>> Linux disk already present: $DISK"
    echo "    (pass --reset to scripts/teddyos-vm.sh if you want a clean image)"
    return 0
  fi

  echo ">>> first-time Linux substrate — this downloads Debian and boots once"
  echo "    for cloud-init (user teddy / password teddyos, ssh -p 2222)."
  echo

  # Create disk + seed by running prepare path: boot headless until ssh answers,
  # then power off so UTM can take the disk.
  (
    cd "$ROOT"
    # Boot in background; teddyos-vm execs qemu — use a subshell + timeout wait.
    ./scripts/teddyos-vm.sh --headless &
    qpid=$!
    trap 'kill "$qpid" 2>/dev/null || true' EXIT

    echo ">>> waiting for ssh on port ${TEDDYOS_VM_SSH_PORT:-2222} (cloud-init can take a few minutes)"
    ok=0
    for i in $(seq 1 180); do
      if ssh -p "${TEDDYOS_VM_SSH_PORT:-2222}" \
          -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
          -o LogLevel=ERROR -o ConnectTimeout=2 -o BatchMode=yes \
          "${TEDDYOS_VM_USER:-teddy}@localhost" true 2>/dev/null; then
        ok=1
        break
      fi
      # If qemu died early, surface that instead of spinning.
      if ! kill -0 "$qpid" 2>/dev/null; then
        wait "$qpid" || true
        echo "error: guest qemu exited before ssh came up" >&2
        exit 1
      fi
      sleep 2
    done
    if [[ "$ok" != 1 ]]; then
      echo "error: ssh never answered — check serial of the headless qemu" >&2
      kill "$qpid" 2>/dev/null || true
      exit 1
    fi
    echo ">>> cloud-init ready — shutting guest down for UTM"
    ssh -p "${TEDDYOS_VM_SSH_PORT:-2222}" \
      -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o LogLevel=ERROR \
      "${TEDDYOS_VM_USER:-teddy}@localhost" 'sudo poweroff' 2>/dev/null || true
    # Wait for qemu to exit after poweroff.
    for _ in $(seq 1 60); do
      kill -0 "$qpid" 2>/dev/null || break
      sleep 1
    done
    kill "$qpid" 2>/dev/null || true
    wait "$qpid" 2>/dev/null || true
    trap - EXIT
  )

  [[ -f "$DISK" ]] || {
    echo "error: expected $DISK after init" >&2
    exit 1
  }
  echo ">>> Linux disk ready: $(du -h "$DISK" | awk '{print $1}')"
}

linux_provision() {
  ssh -p "${TEDDYOS_VM_SSH_PORT:-2222}" \
      -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o LogLevel=ERROR -o ConnectTimeout=3 -o BatchMode=yes \
      "${TEDDYOS_VM_USER:-teddy}@localhost" true 2>/dev/null || {
    echo "error: guest not reachable on ssh port ${TEDDYOS_VM_SSH_PORT:-2222}" >&2
    echo "       start headless QEMU first (port-forward works there, not always under UTM):" >&2
    echo "         ./scripts/teddyos-vm.sh --headless" >&2
    echo "       then re-run: make linux-provision" >&2
    exit 1
  }
  ./scripts/teddyos-provision.sh
}

linux_desktop() {
  if [[ ! -f "$DISK" ]]; then
    cat <<MSG >&2
error: no Linux disk at $DISK

This machine has never been provisioned for the Linux daily driver.
Freestanding kernel ISOs (os.iso / make kernel-utm) are not the product.

  make linux-init        # download Debian + first boot (once)
  make linux-provision   # optional: GNOME+Chromium via headless QEMU
  make desktop           # open Linux teddyOS in UTM

MSG
    exit 1
  fi

  if pgrep -f "qemu-system-aarch64.*${DISK}" >/dev/null 2>&1; then
    echo "error: headless qemu still owns $DISK — shut it down first" >&2
    echo "       (ssh in and 'sudo poweroff', or kill the qemu process)" >&2
    exit 1
  fi

  TEDDYOS_UTM_START="$START" ./scripts/teddyos-utm.sh
}

case "$MODE" in
  init) linux_init ;;
  provision) linux_provision ;;
  desktop) linux_desktop ;;
esac
