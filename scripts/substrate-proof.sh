#!/usr/bin/env bash
# Prove the product layer runs on a Linux guest.
#
# docs/linux-os-doc-v02.md step 2: "Prove the substrate: minimal Linux image,
# our shell, bridge over vsock or a unix socket." This is that, end to end and
# unattended:
#
#   Alpine aarch64, booted under Apple's hypervisor (HVF, native speed)
#     -> logs in
#     -> fetches linux/agent-shell from the host over user-mode networking
#     -> runs it against the REAL host bridge at 10.0.2.2:7420
#     -> prints results that came from the host's indexes
#
# If that works, everything above the kernel - the capability model, the agent,
# the multi-source search, the connectors - is portable, and the migration is
# carrying a working product rather than a hope.
#
# It deliberately uses the same bridge and the same client shipped in linux/.
# A proof against a mock would prove nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${LINUX_VM_WORK:-$ROOT/.linux-vm}"
ISO="$WORK/alpine-aarch64.iso"
FW="${QEMU_AARCH64_FW:-/opt/homebrew/share/qemu/edk2-aarch64-code.fd}"
LOG="$WORK/substrate-proof.log"
VARS="$WORK/vars-proof.fd"
# The guest reaches the host's loopback here under QEMU user networking.
HOST_FROM_GUEST="${HOST_FROM_GUEST:-10.0.2.2}"
BRIDGE_PORT="${BRIDGE_PORT:-7420}"
SERVE_PORT="${SERVE_PORT:-8099}"
# Default to something the built-in corpus answers, so the proof does not
# depend on a 65MB portal sync being present. Pass a query to override.
QUERY="${1:-capability}"

[[ -f "$ISO" ]] || { echo "error: $ISO missing - run 'make linux-vm' once to fetch it" >&2; exit 1; }
[[ -f "$FW" ]] || { echo "error: aarch64 UEFI firmware missing at $FW" >&2; exit 1; }

cleanup() {
  [[ -n "${SERVE_PID:-}" ]] && kill "$SERVE_PID" 2>/dev/null || true
  [[ -n "${QEMU_PID:-}" ]] && kill "$QEMU_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo ">>> bridge"
"$ROOT/scripts/ensure-bridge.sh" >/dev/null
nc -z 127.0.0.1 "$BRIDGE_PORT" || { echo "error: bridge not listening" >&2; exit 1; }
echo "    listening on 127.0.0.1:$BRIDGE_PORT"

# The Alpine live ISO is read-only and has no compiler; the client has to
# arrive from somewhere. Serving linux/ over HTTP is the least machinery.
echo ">>> serving $ROOT/linux on :$SERVE_PORT"
python3 -m http.server "$SERVE_PORT" --bind 127.0.0.1 --directory "$ROOT/linux" >/dev/null 2>&1 &
SERVE_PID=$!

rm -f "$LOG"
dd if=/dev/zero of="$VARS" bs=1m count=64 2>/dev/null

echo ">>> booting alpine under hvf"
{
  # Wait for the login prompt, then drive a shell. Timings are generous
  # because a failure here is indistinguishable from a slow boot.
  sleep "${PROOF_LOGIN_DELAY:-30}"
  printf 'root\n'
  sleep 4
  # Bring up networking. The live ISO leaves eth0 down.
  printf 'ip link set eth0 up; udhcpc -i eth0 -q -n >/dev/null 2>&1; echo NET_UP\n'
  sleep 12
  printf 'wget -q -O /tmp/agent-shell http://%s:%s/agent-shell && chmod +x /tmp/agent-shell && echo GOT_CLIENT\n' \
    "$HOST_FROM_GUEST" "$SERVE_PORT"
  sleep 8
  printf 'echo ---BEGIN---; BRIDGE_ADDR=%s:%s /tmp/agent-shell %s; echo "EXIT=$?"; echo ---END---\n' \
    "$HOST_FROM_GUEST" "$BRIDGE_PORT" "$QUERY"
  sleep "${PROOF_RUN_SECS:-45}"
} | qemu-system-aarch64 \
  -M virt,highmem=on \
  -accel hvf -cpu host -smp 4 -m 2048 \
  -drive "if=pflash,format=raw,readonly=on,file=$FW" \
  -drive "if=pflash,format=raw,file=$VARS" \
  -cdrom "$ISO" \
  -display none -serial stdio \
  -device virtio-rng-pci \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
  -no-reboot > "$LOG" 2>&1 &
QEMU_PID=$!
wait "$QEMU_PID" 2>/dev/null || true

echo
echo ">>> what the guest printed"
sed -n '/---BEGIN---/,/---END---/p' "$LOG" | tr -d '\000' | sed 's/\r//' || true

echo
fail() { echo "SUBSTRATE PROOF FAILED: $1"; echo "full log: $LOG"; exit 1; }
grep -aq NET_UP "$LOG"      || fail "guest never brought up networking"
grep -aq GOT_CLIENT "$LOG"  || fail "guest could not fetch agent-shell from the host"
grep -aq -- "---BEGIN---" "$LOG" || fail "the client never ran"
grep -aq "EXIT=0" "$LOG"    || fail "agent-shell exited non-zero inside the guest"
# A run that prints nothing but exits 0 would be a false pass.
grep -aqE '^ *[0-9]+\. ' "$LOG" || fail "no numbered result rows - the client returned nothing"

echo "SUBSTRATE PROOF PASSED"
echo "  a Linux guest fetched the shipped client and queried the real host bridge"
echo "  full log: $LOG"
