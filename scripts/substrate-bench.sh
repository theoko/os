#!/usr/bin/env bash
# Measure the emulation tax, so the substrate decision rests on a number.
#
# docs/linux-os-doc-v02.md claims the frame ceiling comes from EMULATING
# x86-64 rather than from anything in our draw path: a full-screen present
# costs 67,468k cycles (~22.5ms, ~44fps) under QEMU's TCG on Apple Silicon.
# If that is right, the same guest under hardware virtualisation should be
# dramatically faster, and porting to a substrate that can be hardware
# virtualised is worth its cost. If it is wrong, the migration premise is
# wrong and we should find that out before deleting 2,700 lines of drivers.
#
# The test: boot the identical Alpine aarch64 image twice - once under HVF
# (Apple's hypervisor, native execution) and once under TCG (instruction
# emulation, what our x86 guest lives with) - and time how long each takes to
# reach the same milestone on the serial console. Same image, same devices,
# same milestone; the only variable is the accelerator.
#
# Boot time is not frame time. It is the same *tax* measured on a workload
# that can be automated, which is what the claim is about.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${LINUX_VM_WORK:-$ROOT/.linux-vm}"
ISO="$WORK/alpine-aarch64.iso"
FW="${QEMU_AARCH64_FW:-/opt/homebrew/share/qemu/edk2-aarch64-code.fd}"
# What "booted" means. Alpine prints this once userspace is up.
MILESTONE="${SUBSTRATE_MILESTONE:-Welcome to Alpine Linux}"
TIMEOUT="${SUBSTRATE_TIMEOUT:-300}"

[[ -f "$ISO" ]] || { echo "error: $ISO missing - run 'make linux-vm' once to fetch it" >&2; exit 1; }
[[ -f "$FW" ]] || { echo "error: aarch64 UEFI firmware missing at $FW" >&2; exit 1; }

run() {
  local accel="$1"
  local log="$WORK/boot-$accel.log"
  local vars="$WORK/vars-$accel.fd"
  rm -f "$log"
  dd if=/dev/zero of="$vars" bs=1m count=64 2>/dev/null

  local start
  start=$(date +%s)
  qemu-system-aarch64 \
    -M virt,highmem=on \
    -accel "$accel" \
    -cpu "$([ "$accel" = hvf ] && echo host || echo cortex-a72)" \
    -smp 4 -m 2048 \
    -drive "if=pflash,format=raw,readonly=on,file=$FW" \
    -drive "if=pflash,format=raw,file=$vars" \
    -cdrom "$ISO" \
    -display none -serial "file:$log" \
    -device virtio-rng-pci \
    -no-reboot >/dev/null 2>&1 &
  local pid=$!

  local elapsed=0
  while [[ $elapsed -lt $TIMEOUT ]]; do
    if [[ -s "$log" ]] && grep -aq "$MILESTONE" "$log" 2>/dev/null; then
      break
    fi
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
    elapsed=$(( $(date +%s) - start ))
  done
  local took=$(( $(date +%s) - start ))
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true

  if grep -aq "$MILESTONE" "$log" 2>/dev/null; then
    echo "$took"
  else
    echo "TIMEOUT"
  fi
}

# Boot time is dominated by device probing and waiting, so it understates the
# tax badly. This drives the guest to a shell and times a bulk memory copy -
# the same shape as the full-screen present that costs us 22.5ms, and the
# workload instruction emulation handles worst.
bench() {
  local accel="$1"
  local log="$WORK/bench-$accel.log"
  local vars="$WORK/vars-$accel.fd"
  rm -f "$log"
  dd if=/dev/zero of="$vars" bs=1m count=64 2>/dev/null

  # Log in and copy 512MB through the kernel, then print a sentinel. busybox
  # dd reports throughput on stderr, which shares the console.
  {
    sleep "${BENCH_LOGIN_DELAY:-25}"
    printf 'root\n'
    sleep 3
    printf 'dd if=/dev/zero of=/dev/null bs=1M count=512; echo BENCH_DONE\n'
    sleep "${BENCH_RUN_SECS:-120}"
  } | qemu-system-aarch64 \
    -M virt,highmem=on \
    -accel "$accel" \
    -cpu "$([ "$accel" = hvf ] && echo host || echo cortex-a72)" \
    -smp 4 -m 2048 \
    -drive "if=pflash,format=raw,readonly=on,file=$FW" \
    -drive "if=pflash,format=raw,file=$vars" \
    -cdrom "$ISO" \
    -display none -serial stdio -device virtio-rng-pci -no-reboot \
    > "$log" 2>&1 || true

  grep -a "copied" "$log" | tail -1
}

if [[ "${1:-}" == "--memcpy" ]]; then
  echo ">>> bulk memory copy inside the guest (512MB)"
  echo ">>> hvf: $(bench hvf)"
  echo ">>> tcg: $(bench tcg)"
  echo "logs: $WORK/bench-hvf.log $WORK/bench-tcg.log"
  exit 0
fi

echo ">>> milestone: '$MILESTONE' on the serial console"
echo ">>> hvf (hardware virtualisation - what a Linux substrate would get)"
hvf=$(run hvf)
echo "    ${hvf}s"
echo ">>> tcg (instruction emulation - what our x86 guest lives with today)"
tcg=$(run tcg)
echo "    ${tcg}s"

echo
if [[ "$hvf" == TIMEOUT || "$tcg" == TIMEOUT ]]; then
  echo "inconclusive: hvf=$hvf tcg=$tcg (raise SUBSTRATE_TIMEOUT, or check $WORK/boot-*.log)"
  exit 1
fi
if [[ "$hvf" -eq 0 ]]; then hvf=1; fi
echo "emulation tax: ${tcg}s vs ${hvf}s = $(( tcg / hvf ))x slower under TCG"
echo "logs: $WORK/boot-hvf.log $WORK/boot-tcg.log"
