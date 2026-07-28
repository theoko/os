#!/usr/bin/env bash
# The teddyOS daily-driver substrate: aarch64 Debian under Apple's hypervisor.
#
# This is not `linux-vm.sh`. That one boots a live Alpine ISO to prove HVF is
# real (docs/linux-os-doc-v02.md) and forgets everything on shutdown. This one
# is meant to be lived in: a persistent disk, a real user, enough RAM to run a
# browser, and a machine that comes back the way you left it.
#
# Route A of the daily-driver plan — full-screen Linux on this Mac at native
# speed, with the teddyOS shell on top of it. Route C (bare metal on dedicated
# hardware) reuses everything above the kernel; only the launcher is thrown away.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${TEDDYOS_VM_WORK:-$ROOT/.teddyos-vm}"

BASE_URL="${TEDDYOS_VM_IMAGE_URL:-https://cloud.debian.org/images/cloud/trixie/latest/debian-13-generic-arm64.qcow2}"
SUMS_URL="${TEDDYOS_VM_SUMS_URL:-https://cloud.debian.org/images/cloud/trixie/latest/SHA512SUMS}"
BASE="$WORK/base.qcow2"
DISK="$WORK/disk.qcow2"
SEED="$WORK/seed.iso"
VARS="$WORK/vars.fd"
FW="${QEMU_AARCH64_FW:-/opt/homebrew/share/qemu/edk2-aarch64-code.fd}"

DISK_SIZE="${TEDDYOS_VM_DISK:-120G}"
MEM="${TEDDYOS_VM_MEM:-12288}"
CPUS="${TEDDYOS_VM_CPUS:-8}"
XRES="${TEDDYOS_VM_XRES:-1920}"
YRES="${TEDDYOS_VM_YRES:-1200}"
SSH_PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"

HEADLESS=0
RESET=0
for arg in "$@"; do
  case "$arg" in
    --headless) HEADLESS=1 ;;
    --reset)    RESET=1 ;;
    --help|-h)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      cat <<'USAGE'

usage: scripts/teddyos-vm.sh [--headless] [--reset]

  --headless   no window; serial console on this terminal, ssh on $SSH_PORT
  --reset      discard the working disk and re-provision from the base image

  ssh in with:  ssh -p 2222 teddy@localhost

env: TEDDYOS_VM_MEM (MB)  TEDDYOS_VM_CPUS  TEDDYOS_VM_DISK  TEDDYOS_VM_XRES/YRES
USAGE
      exit 0 ;;
    *) echo "error: unknown argument $arg (try --help)" >&2; exit 2 ;;
  esac
done

mkdir -p "$WORK"

[[ -f "$FW" ]] || {
  echo "error: aarch64 UEFI firmware missing at $FW" >&2
  echo "       brew install qemu" >&2
  exit 1
}

qemu-system-aarch64 -accel help 2>/dev/null | grep -q hvf || {
  echo "error: this QEMU has no hvf accelerator — without it the guest is" >&2
  echo "       emulated and unusable as a daily driver. brew install qemu" >&2
  exit 1
}

# --- base image -------------------------------------------------------------
# Verified, not just downloaded. A truncated or substituted cloud image is the
# kind of failure that shows up later as something unrelated and unexplainable,
# which is exactly the debt make-usb.sh refuses to take on.
if [[ ! -f "$BASE" ]]; then
  echo ">>> fetching $(basename "$BASE_URL")"
  curl -fL --progress-bar -o "$BASE.part" "$BASE_URL"
  mv "$BASE.part" "$BASE"
fi

if [[ ! -f "$WORK/.base-verified" ]]; then
  echo ">>> verifying base image"
  curl -fsL -o "$WORK/SHA512SUMS" "$SUMS_URL"
  want="$(awk -v f="$(basename "$BASE_URL")" '$2 == f {print $1}' "$WORK/SHA512SUMS")"
  [[ -n "$want" ]] || { echo "error: $(basename "$BASE_URL") not listed in SHA512SUMS" >&2; exit 1; }
  got="$(shasum -a 512 "$BASE" | awk '{print $1}')"
  if [[ "$want" != "$got" ]]; then
    echo "error: base image checksum mismatch — refusing to boot it" >&2
    echo "  want $want" >&2
    echo "  got  $got" >&2
    echo "  rm $BASE and re-run" >&2
    exit 1
  fi
  touch "$WORK/.base-verified"
  echo "    ok"
fi

# --- working disk -----------------------------------------------------------
if [[ "$RESET" == 1 && -f "$DISK" ]]; then
  echo ">>> --reset: discarding $DISK"
  rm -f "$DISK" "$VARS" "$SEED"
fi

if [[ ! -f "$DISK" ]]; then
  echo ">>> provisioning $DISK ($DISK_SIZE)"
  # A copy, not a backing file: the daily driver should not break because the
  # base image underneath it was moved or re-downloaded.
  cp "$BASE" "$DISK"
  qemu-img resize "$DISK" "$DISK_SIZE" >/dev/null
fi

# --- cloud-init seed --------------------------------------------------------
# Regenerated whenever it is missing, so `--reset` re-provisions the user too.
if [[ ! -f "$SEED" ]]; then
  echo ">>> building cloud-init seed"
  seeddir="$(mktemp -d)"
  trap 'rm -rf "$seeddir"' EXIT

  authkeys=""
  for pub in "$HOME"/.ssh/id_ed25519.pub "$HOME"/.ssh/id_rsa.pub; do
    [[ -f "$pub" ]] && authkeys="$authkeys      - $(cat "$pub")"$'\n'
  done
  if [[ -z "$authkeys" ]]; then
    echo "    note: no ssh public key found; password login only"
    authkeys="      []"
  fi

  cat > "$seeddir/meta-data" <<META
instance-id: teddyos-01
local-hostname: teddyos
META

  cat > "$seeddir/user-data" <<USERDATA
#cloud-config
hostname: teddyos
users:
  - name: $VM_USER
    groups: [sudo, audio, video]
    sudo: "ALL=(ALL) NOPASSWD:ALL"
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
$authkeys
chpasswd:
  expire: false
  users:
    - name: $VM_USER
      password: teddyos
      type: text
ssh_pwauth: true
package_update: true
packages:
  - curl
  - git
  - ca-certificates
USERDATA

  # cloud-init's NoCloud datasource keys off the volume label, which must be
  # CIDATA. hdiutil is the only ISO writer guaranteed present on macOS.
  rm -f "$SEED"
  hdiutil makehybrid -quiet -iso -joliet -default-volume-name CIDATA \
    -o "$SEED" "$seeddir"
  # makehybrid appends .iso when the name lacks it; tolerate either result.
  [[ -f "$SEED" ]] || mv "$SEED.iso" "$SEED"
  rm -rf "$seeddir"
  trap - EXIT
fi

# Writable UEFI variable store; the firmware image itself is read-only.
[[ -f "$VARS" ]] || dd if=/dev/zero of="$VARS" bs=1m count=64 2>/dev/null

# --- boot -------------------------------------------------------------------
display=(-device virtio-gpu-pci,xres="$XRES",yres="$YRES"
         -device virtio-keyboard-pci
         -device virtio-tablet-pci
         -display cocoa)
if [[ "$HEADLESS" == 1 ]]; then
  display=(-display none -serial mon:stdio)
fi

echo ">>> booting teddyos  ${CPUS} cpu / $((MEM / 1024))G ram / $DISK_SIZE disk"
echo "    ssh -p $SSH_PORT $VM_USER@localhost   (password: teddyos)"

exec qemu-system-aarch64 \
  -M virt,highmem=on \
  -accel hvf \
  -cpu host \
  -smp "$CPUS" \
  -m "$MEM" \
  -drive "if=pflash,format=raw,readonly=on,file=$FW" \
  -drive "if=pflash,format=raw,file=$VARS" \
  -drive "if=virtio,format=qcow2,file=$DISK" \
  -drive "if=virtio,format=raw,file=$SEED,readonly=on" \
  -device virtio-rng-pci \
  -netdev "user,id=net0,hostfwd=tcp::$SSH_PORT-:22" \
  -device virtio-net-pci,netdev=net0 \
  "${display[@]}"
