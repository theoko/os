#!/usr/bin/env bash
# Substrate proof: boot aarch64 Linux under Apple's hypervisor and measure it.
#
# This exists to test one claim before any porting starts — that the frame
# ceiling comes from EMULATING x86-64, not from our kernel. See
# docs/linux-os-doc-v02.md. If HVF does not deliver here, the migration
# premise is wrong and we should know that now rather than after the port.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${LINUX_VM_WORK:-$ROOT/.linux-vm}"
ISO_URL="${ALPINE_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/aarch64/alpine-virt-3.21.0-aarch64.iso}"
ISO="$WORK/alpine-aarch64.iso"
FW="${QEMU_AARCH64_FW:-/opt/homebrew/share/qemu/edk2-aarch64-code.fd}"
VARS="$WORK/vars.fd"

mkdir -p "$WORK"

if [[ ! -f "$FW" ]]; then
  echo "error: aarch64 UEFI firmware missing at $FW" >&2
  echo "       brew install qemu" >&2
  exit 1
fi

if ! qemu-system-aarch64 -accel help 2>/dev/null | grep -q hvf; then
  echo "error: this QEMU has no hvf accelerator — the whole point of this test" >&2
  exit 1
fi

if [[ ! -f "$ISO" ]]; then
  echo ">>> fetching $ISO_URL"
  curl -fL --progress-bar -o "$ISO.part" "$ISO_URL"
  mv "$ISO.part" "$ISO"
fi
echo ">>> iso: $(du -h "$ISO" | awk '{print $1}')"

# Writable copy of the UEFI variable store; the firmware image is read-only.
[[ -f "$VARS" ]] || { dd if=/dev/zero of="$VARS" bs=1m count=64 2>/dev/null; }

# virtio-gpu + virtio-keyboard/tablet is the combination the port would target:
# no PS/2, no UHCI, no port I/O — the drivers we would delete.
exec qemu-system-aarch64 \
  -M virt,highmem=on \
  -accel "${LINUX_VM_ACCEL:-hvf}" \
  -cpu host \
  -smp "${LINUX_VM_CPUS:-4}" \
  -m "${LINUX_VM_MEM:-2048}" \
  -drive "if=pflash,format=raw,readonly=on,file=$FW" \
  -drive "if=pflash,format=raw,file=$VARS" \
  -cdrom "$ISO" \
  -device virtio-gpu-pci \
  -device virtio-keyboard-pci \
  -device virtio-tablet-pci \
  -device virtio-rng-pci \
  -netdev user,id=net0 \
  -device virtio-net-pci,netdev=net0 \
  "${@:-}"
