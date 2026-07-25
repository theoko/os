#!/usr/bin/env bash
# Write os.iso to a USB stick so it can boot real x86-64 hardware.
#
# Why this exists: on an Apple Silicon Mac nothing can *virtualise* an x86
# guest. VirtualBox and Parallels both refuse outright
# (VBOX_E_PLATFORM_ARCH_NOT_SUPPORTED), and UTM only works because QEMU
# emulates every instruction in software - which is also why it will never hit
# 60fps. Real hardware is the only way to see this OS run at full speed.
#
# This script writes to a raw block device, so every guard here is load-bearing:
#   * refuses anything the system reports as internal
#   * refuses anything currently mounted as a system volume
#   * requires the device to be named explicitly - there is no autodetect
#   * requires the operator to retype the device before anything is written
#   * verifies the bytes afterwards rather than trusting dd's exit code
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ISO="${IMAGE_NAME:-os}.iso"
DEVICE="${DEVICE:-}"
DRY_RUN="${DRY_RUN:-0}"

die() { echo "error: $*" >&2; exit 1; }

list_candidates() {
  echo "External physical disks:"
  echo
  diskutil list external physical 2>/dev/null || true
  echo
  echo "Write with:  DEVICE=/dev/diskN make usb"
  echo "Pick the whole disk (/dev/disk4), not a partition (/dev/disk4s1)."
}

# Everything that must be true before we are allowed to write to $1.
check_device() {
  local dev="$1"
  [[ "$dev" == /dev/disk* ]] || die "$dev is not a /dev/diskN device"
  [[ "$dev" =~ ^/dev/r?disk[0-9]+$ ]] || die "$dev looks like a partition - pass the whole disk"
  [[ -b "$dev" || -c "$dev" ]] || die "$dev does not exist"

  local info
  info="$(diskutil info "$dev" 2>/dev/null)" || die "cannot read $dev - is it plugged in?"

  # An internal disk is the boot drive or close to it. Never.
  if grep -qE '^ *(Device Location|Internal): +Internal' <<<"$info"; then
    die "$dev is an internal disk. Refusing."
  fi
  if grep -qE '^ *Internal: +Yes' <<<"$info"; then
    die "$dev is an internal disk. Refusing."
  fi

  # A disk holding a mounted system volume is not a spare stick.
  if diskutil info "$dev" 2>/dev/null | grep -q 'Mount Point: */$'; then
    die "$dev holds the running system volume. Refusing."
  fi

  local size_label
  size_label="$(grep -E '^ *Disk Size:' <<<"$info" | head -1 | sed 's/^ *Disk Size: *//')"
  echo "$size_label"
}

if [[ "${1:-}" == "--list" || -z "$DEVICE" ]]; then
  list_candidates
  [[ -z "$DEVICE" && "${1:-}" != "--list" ]] && die "set DEVICE=/dev/diskN"
  exit 0
fi

[[ -f "$ISO" ]] || die "$ISO missing - run 'make iso' first"

size_label="$(check_device "$DEVICE")"
iso_size="$(stat -f%z "$ISO")"

echo
echo "  ISO:    $ISO ($(( iso_size / 1024 / 1024 )) MB)"
echo "  Target: $DEVICE  $size_label"
echo
echo "EVERYTHING on $DEVICE will be destroyed."
echo

if [[ "$DRY_RUN" == "1" ]]; then
  echo "dry run: all checks passed, nothing written"
  exit 0
fi

# Retyping the device is the last guard. A y/n prompt is too easy to answer
# on autopilot when the cost is somebody's backup drive.
read -r -p "Type the device path to confirm: " typed
[[ "$typed" == "$DEVICE" ]] || die "typed '$typed', expected '$DEVICE' - nothing written"

raw="${DEVICE/\/dev\/disk//dev/rdisk}"
echo "unmounting $DEVICE"
diskutil unmountDisk "$DEVICE"

echo "writing (needs sudo for raw disk access)"
sudo dd if="$ISO" of="$raw" bs=4m status=progress
sync

# Verify rather than trusting the exit code: a stick that reports success and
# holds different bytes is the worst outcome, because it fails at boot on
# another machine with no clue why.
echo "verifying"
iso_hash="$(shasum -a 256 "$ISO" | cut -d' ' -f1)"
dev_hash="$(sudo dd if="$raw" bs=1m count=$(( (iso_size + 1048575) / 1048576 )) 2>/dev/null \
  | head -c "$iso_size" | shasum -a 256 | cut -d' ' -f1)"

diskutil eject "$DEVICE" >/dev/null 2>&1 || true

if [[ "$iso_hash" != "$dev_hash" ]]; then
  die "verify failed - the stick does not match the ISO. Do not boot from it."
fi

echo
echo "usb ok: $DEVICE matches $ISO"
echo ">>> Boot the target machine from it. On most PCs that is F12 / F2 / Del."
echo ">>> Secure Boot must be off - the Limine loader is not signed."
echo ">>> See docs/install-os-doc-v01.md, especially the note about USB input."
