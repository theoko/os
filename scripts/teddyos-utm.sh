#!/usr/bin/env bash
# Hand the provisioned teddyOS disk to UTM, which is the only host on this Mac
# with a GPU path.
#
# Why not just add -display to teddyos-vm.sh: Homebrew's qemu is built without
# OpenGL and without virglrenderer —
#
#   $ qemu-system-aarch64 -device help | grep virtio-gpu
#   virtio-gpu-pci            <- no -gl variant
#   $ qemu-system-aarch64 -display help
#   none curses cocoa dbus    <- no gl-capable backend
#
# so a desktop on it renders through llvmpipe on the CPU. UTM 4.7.5 ships
# virglrenderer.1.framework, epoxy.0.framework and GLESv2.framework, and its
# aarch64 build does have virtio-gpu-gl-pci / virtio-ramfb-gl. Same guest disk,
# same hypervisor; the difference is entirely who draws.
#
# teddyos-vm.sh stays useful: it is the headless/scriptable path (ssh, apt,
# provisioning, CI). This one is the screen you sit in front of.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WORK="${TEDDYOS_VM_WORK:-$ROOT/.teddyos-vm}"
SRC_DISK="$WORK/disk.qcow2"
VM_NAME="${TEDDYOS_UTM_NAME:-teddyos}"
MEM="${TEDDYOS_VM_MEM:-12288}"
CPUS="${TEDDYOS_VM_CPUS:-8}"
SSH_PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
# virtio-ramfb-gl rather than virtio-gpu-gl-pci: ramfb gives a framebuffer the
# firmware can draw into, so UEFI and the bootloader are visible. With the pure
# PCI device the window stays black until the guest's DRM driver loads, which
# is indistinguishable from a hang on the one boot where something is wrong.
DISPLAY_HW="${TEDDYOS_UTM_DISPLAY:-virtio-ramfb-gl}"
START="${TEDDYOS_UTM_START:-0}"

UTM_DOCS="$HOME/Library/Containers/com.utmapp.UTM/Data/Documents"
UTM_DIR="$UTM_DOCS/${VM_NAME}.utm"
DEST_DISK="$UTM_DIR/Data/teddyos.qcow2"

if [[ ! "$VM_NAME" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "error: TEDDYOS_UTM_NAME must match [A-Za-z0-9._-]+ (got: $VM_NAME)" >&2
  exit 1
fi
[[ -d /Applications/UTM.app ]] || {
  echo "error: UTM.app not found — brew install --cask utm" >&2; exit 1; }

utm_quit() {
  osascript -e 'tell application "UTM" to quit' >/dev/null 2>&1 || true
  for _ in $(seq 1 40); do
    pgrep -x UTM >/dev/null 2>&1 || return 0
    sleep 0.25
  done
}
utm_open() {
  open -a UTM
  for _ in $(seq 1 40); do
    osascript -e 'tell application "UTM" to count virtual machines' >/dev/null 2>&1 && return 0
    sleep 0.25
  done
}

have_bundle() { [[ -d "$UTM_DIR" && -f "$UTM_DIR/config.plist" ]]; }

# --- which disk is authoritative -------------------------------------------
# Once UTM has booted this VM, the copy inside the bundle is the one with the
# user's work in it. Silently overwriting it from .teddyos-vm/disk.qcow2 —
# which stops changing the moment they stop using teddyos-vm.sh — would delete
# a day's work and look like the VM "reset itself".
COPY_DISK=1
if [[ -f "$DEST_DISK" ]]; then
  if [[ ! -f "$SRC_DISK" ]] || [[ "$DEST_DISK" -nt "$SRC_DISK" ]]; then
    COPY_DISK=0
    echo ">>> keeping the UTM disk (newer than $SRC_DISK)"
  else
    echo "warning: $DEST_DISK exists and is older than the source disk." >&2
    echo "         Overwriting it discards anything done inside UTM since." >&2
    printf "         type the VM name (%s) to overwrite, anything else to keep: " "$VM_NAME" >&2
    read -r confirm
    [[ "$confirm" == "$VM_NAME" ]] || { COPY_DISK=0; echo ">>> keeping the UTM disk"; }
  fi
fi

if [[ "$COPY_DISK" == 1 && ! -f "$SRC_DISK" ]]; then
  echo "error: $SRC_DISK missing — create it first:" >&2
  echo "       ./scripts/teddyos-vm.sh --headless   (then provision, then stop it)" >&2
  exit 1
fi

# A qcow2 opened by a running qemu is mid-write; copying it yields a disk that
# fsck's dirty at best.
if pgrep -f "qemu-system-aarch64.*$SRC_DISK" >/dev/null 2>&1; then
  echo "error: teddyos-vm.sh is still running against $SRC_DISK" >&2
  echo "       shut that guest down before copying its disk" >&2
  exit 1
fi

if [[ -d "$UTM_DIR" && ! -f "$UTM_DIR/config.plist" ]]; then
  echo ">>> removing orphan bundle: $UTM_DIR"
  rm -rf "$UTM_DIR"
fi

# --- create or refresh ------------------------------------------------------
if have_bundle; then
  echo ">>> refreshing existing VM bundle: $UTM_DIR"
  osascript <<EOF >/dev/null 2>&1 || true
tell application "UTM"
  try
    set vm to virtual machine named "$VM_NAME"
    if status of vm is not stopped then
      stop vm by kill
      delay 1
    end if
  end try
end tell
EOF
else
  # Created with no drives; the disk is bundled and declared by the plist patch
  # below. AppleScript drive import is the flakiest part of UTM's bridge and
  # there is no reason to depend on it when we already patch the config.
  utm_open
  osascript <<EOF
tell application "UTM"
  activate
  make new virtual machine with properties {backend:qemu, configuration:{name:"$VM_NAME", architecture:"aarch64", memory:$MEM, hypervisor:true, uefi:true}}
end tell
EOF
fi

for _ in $(seq 1 20); do [[ -d "$UTM_DIR" ]] && break; sleep 0.25; done
[[ -d "$UTM_DIR" ]] || { echo "error: VM bundle was not created" >&2; exit 1; }

osascript <<EOF >/dev/null 2>&1 || true
tell application "UTM"
  try
    set vm to virtual machine named "$VM_NAME"
    if status of vm is not stopped then
      stop vm by kill
      delay 1
    end if
  end try
end tell
EOF

mkdir -p "$UTM_DIR/Data"
if [[ "$COPY_DISK" == 1 ]]; then
  echo ">>> copying disk into the bundle ($(du -h "$SRC_DISK" | awk '{print $1}'))"
  cp -f "$SRC_DISK" "$DEST_DISK"
fi

UTM_DIR="$UTM_DIR" MEM="$MEM" CPUS="$CPUS" SSH_PORT="$SSH_PORT" \
DISPLAY_HW="$DISPLAY_HW" VM_NAME="$VM_NAME" python3 <<'PY'
import os, plistlib, uuid, hashlib
from pathlib import Path

p = Path(os.environ["UTM_DIR"]) / "config.plist"
cfg = plistlib.loads(p.read_bytes())

cfg["Drive"] = [{
    "Identifier": str(uuid.uuid4()).upper(),
    "ImageType": "Disk",
    "ImageName": "teddyos.qcow2",
    "Interface": "VirtIO",
    "InterfaceVersion": 1,
    "ReadOnly": False,
}]

qemu = cfg.setdefault("QEMU", {})
qemu["UEFIBoot"] = True
# The whole point. Without HVF this is TCG and the guest is a slideshow.
qemu["Hypervisor"] = True
qemu["RNGDevice"] = True
qemu["BalloonDevice"] = True

system = cfg.setdefault("System", {})
system["MemorySize"] = int(os.environ["MEM"])
system["CPUCount"] = int(os.environ["CPUS"])
system["Architecture"] = "aarch64"
system["Target"] = "virt"

cfg["Display"] = [{
    "Hardware": os.environ["DISPLAY_HW"],
    # Let the guest resize to the window instead of pillarboxing a fixed mode.
    # GNOME's mutter honours the virtio-gpu hotplug event, so dragging the
    # window edge changes the desktop resolution rather than scaling it.
    "DynamicResolution": True,
    "NativeResolution": True,
    "UpscalingFilter": "Linear",
    "DownscalingFilter": "Linear",
}]

# MacAddress and IsolateFromHost are REQUIRED, not optional. UTM's decoder
# rejects a Network entry missing either one, and rejecting the entry means
# rejecting the whole config: the VM vanishes from the library with no error
# anywhere — not in `utmctl list`, not in Console, not in a log. The bundle and
# the registry row both survive, which makes it look like a UTM bug rather than
# a malformed key. Found by bisecting the patch one block at a time.
#
# The MAC is derived from the VM name rather than random so that re-running
# this script does not hand the guest a new NIC identity every time.
mac_src = hashlib.sha256(os.environ["VM_NAME"].encode()).digest()
mac = "02:" + ":".join(f"{b:02X}" for b in mac_src[:5])

# NOTE ON "Shared": UTM implements it with `-netdev vmnet-shared`, not QEMU
# user-mode networking, and vmnet gives the guest its own address on a host
# bridge — so PortForward is silently ignored. ssh to the guest's own IP
# instead; the entry below is kept only for the modes that do honour it.
#   $ cat /var/db/dhcpd_leases        # name=teddyos -> ip_address=...
cfg["Network"] = [{
    "Mode": "Shared",
    "Hardware": "virtio-net-pci",
    "IsolateFromHost": False,
    "MacAddress": mac,
    "PortForward": [{
        "Protocol": "TCP",
        "HostAddress": "127.0.0.1",
        "HostPort": int(os.environ["SSH_PORT"]),
        "GuestPort": 22,
    }],
}]

cfg["Sound"] = [{"Hardware": "intel-hda"}]
cfg["Serial"] = [{"Mode": "Ptty", "Target": "Auto"}]

# USB tablet, so the pointer is absolute and the cursor does not need grabbing.
cfg.setdefault("Input", {})["UsbBusSupport"] = "Usb3_0"

p.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
print(f"    display  {os.environ['DISPLAY_HW']} (GL)")
print(f"    cpu/mem  {os.environ['CPUS']} / {int(os.environ['MEM'])//1024}G, hypervisor on")
print(f"    ssh      127.0.0.1:{os.environ['SSH_PORT']} -> guest:22")
PY

# Reload so UTM reads the patched plist rather than its in-memory copy.
utm_quit
utm_open

[[ "$START" == "1" ]] && \
  osascript -e "tell application \"UTM\" to start virtual machine named \"$VM_NAME\"" >/dev/null

cat <<DONE

>>> '$VM_NAME' is in UTM. Double-click it to open the guest display.
    The thumbnail in the library list is a still, not the running screen.

    Full screen is what makes this a daily driver rather than a window:
    UTM window -> green button, or ctrl-cmd-F.
DONE
