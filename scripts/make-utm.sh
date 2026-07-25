#!/usr/bin/env bash
# Create or refresh a UTM QEMU VM that boots os.iso (x86_64 TCG).
#
# UTM is sandboxed and often drops AppleScript file bookmarks. We:
#  1) stage the ISO under UTM's container Documents/Public
#  2) create the VM
#  3) copy ISO into the .utm/Data bundle and set ImageName in config.plist
#  4) quit/reopen UTM so it reloads the patched config
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VM_NAME="${UTM_VM_NAME:-os}"
ISO="${IMAGE_NAME:-os}.iso"
START="${UTM_START:-0}"
# COM2 defaults ON. A TCP client with nothing listening is harmless — the guest
# reports the bridge offline, which is true. Gating it off by default meant a
# plain `make utm` produced a VM that could never reach the bridge.
BRIDGE="${UTM_BRIDGE:-1}"
BRIDGE_ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
UTM_DOCS="$HOME/Library/Containers/com.utmapp.UTM/Data/Documents"
UTM_DIR="$UTM_DOCS/${VM_NAME}.utm"
STAGED="$UTM_DOCS/Public/os-boot.iso"

# VM_NAME and paths are interpolated into AppleScript string literals below; a
# quote or backslash would break (or inject into) the script, so constrain the
# name and escape the path.
if [[ ! "$VM_NAME" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "error: UTM_VM_NAME must match [A-Za-z0-9._-]+ (got: $VM_NAME)" >&2
  exit 1
fi
as_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }
STAGED_AS="$(as_escape "$STAGED")"

# Quit/reopen UTM with real synchronization — fixed sleeps race a slow quit and
# the AppleScript bridge coming back up.
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
    if osascript -e 'tell application "UTM" to count virtual machines' >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
}

if [[ ! -f "$ISO" ]]; then
  echo "error: $ISO missing — run 'make iso' first" >&2
  exit 1
fi
if [[ ! -d /Applications/UTM.app ]]; then
  echo "error: UTM.app not found — brew install --cask utm" >&2
  exit 1
fi

mkdir -p "$UTM_DOCS/Public"
cp -f "$ISO" "$STAGED"

have_bundle() { [[ -d "$UTM_DIR" && -f "$UTM_DIR/config.plist" ]]; }

# Orphan on-disk bundles (ISO/screenshot, no config) block AppleScript create.
if [[ -d "$UTM_DIR" && ! -f "$UTM_DIR/config.plist" ]]; then
  echo "removing orphan bundle: $UTM_DIR"
  rm -rf "$UTM_DIR"
fi

# Prefer refresh over delete+create. Ghost library rows (name registered, bundle
# gone) make `make new` fail with -2700 and `utmctl delete` cannot remove them
# either — scrub Name==$VM_NAME from UTM's preferences, then create once.
listed="$(utmctl list 2>/dev/null | awk -v n="$VM_NAME" 'NR>1 && $3==n {print $1}' || true)"

if have_bundle; then
  echo "refreshing existing VM bundle: $UTM_DIR"
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
elif [[ -n "$listed" ]]; then
  echo "scrubbing ghost UTM library entries named '$VM_NAME'"
  utm_quit
  UTM_VM_NAME="$VM_NAME" python3 <<'PY'
import os, plistlib
from pathlib import Path
name = os.environ["UTM_VM_NAME"]
p = Path.home() / "Library/Containers/com.utmapp.UTM/Data/Library/Preferences/com.utmapp.UTM.plist"
cfg = plistlib.loads(p.read_bytes())
changed = False
for k, v in list(cfg.items()):
    if isinstance(v, list) and v and isinstance(v[0], dict) and "UUID" in v[0]:
        keep = [i for i in v if i.get("Name") != name]
        if len(keep) != len(v):
            cfg[k] = keep
            changed = True
if changed:
    p.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
    print("purged ghost entries from", p)
PY
  utm_open
  osascript <<EOF
set isoPath to POSIX file "$STAGED_AS"
tell application "UTM"
  activate
  make new virtual machine with properties {backend:qemu, configuration:{name:"$VM_NAME", architecture:"x86_64", memory:1024, hypervisor:false, uefi:true, displays:{{hardware:"virtio-vga"}}, drives:{{removable:true, source:isoPath}}}}
end tell
EOF
else
  osascript <<EOF
set isoPath to POSIX file "$STAGED_AS"
tell application "UTM"
  activate
  make new virtual machine with properties {backend:qemu, configuration:{name:"$VM_NAME", architecture:"x86_64", memory:1024, hypervisor:false, uefi:true, displays:{{hardware:"virtio-vga"}}, drives:{{removable:true, source:isoPath}}}}
end tell
EOF
fi

for _ in $(seq 1 20); do
  [[ -d "$UTM_DIR" ]] && break
  sleep 0.25
done
[[ -d "$UTM_DIR" ]] || { echo "error: VM bundle missing" >&2; exit 1; }

osascript <<EOF
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
cp -f "$STAGED" "$UTM_DIR/Data/os.iso"

UTM_DIR="$UTM_DIR" UTM_BRIDGE="$BRIDGE" OS_MCP_BRIDGE_ADDR="$BRIDGE_ADDR" python3 <<'PY'
import plistlib, uuid, os
from pathlib import Path
p = Path(os.environ["UTM_DIR"]) / "config.plist"
cfg = plistlib.loads(p.read_bytes())
cfg["Drive"] = [{
    "Identifier": str(uuid.uuid4()).upper(),
    "ImageType": "CD",
    "ImageName": "os.iso",
    "Interface": "IDE",
    "InterfaceVersion": 1,
    "ReadOnly": True,
}]
cfg.setdefault("QEMU", {})["UEFIBoot"] = True
cfg["QEMU"]["Hypervisor"] = False
# PS/2 on for keyboards; pointer comes from usb-tablet (guest UHCI HID driver).
cfg["QEMU"]["PS2Controller"] = True
# Turn UTM's own USB input off and supply our own pointer instead.
#
# UTM's USB bus is hostile to a small guest driver: `-usb` on q35 builds an ICH9
# set at 00:1d.x (EHCI + 3 UHCI companions) carrying usb-tablet on usb-bus.0,
# while UTM adds a *second* explicit ich9-usb-ehci1 at 00:01.0 hosting three
# usb-redir stubs. Two EHCIs, four UHCIs and several decoy HID devices — the
# guest ends up enumerating a redirect stub or a keyboard instead of the tablet.
cfg.setdefault("Input", {})
cfg["Input"]["UsbBusSupport"] = "Disabled"
cfg["Input"]["UsbSharing"] = False
# One dedicated UHCI controller with exactly one device on it. Full speed by
# construction, no EHCI to hand the port off from, nothing else to confuse the
# probe. Verified end to end: `info usb` reports 12 Mb/s, the guest logs
# "usb-tablet ready", and the cursor tracks absolute input.
#
# AdditionalArguments MUST be a flat list of plain strings, one per argv token.
# A dict entry (e.g. {"ArgumentString": ...}) or a single "flag value" string
# fails to decode and UTM silently drops the VM from its library entirely.
EXTRA_ARGS = [
    "-device", "piix3-usb-uhci,id=uhci0",
    "-device", "usb-tablet,bus=uhci0.0",
]
# Set outright rather than merging token-by-token: EXTRA_ARGS repeats "-device",
# so a per-token dedup would collapse the two devices into one.
cfg.setdefault("QEMU", {})["AdditionalArguments"] = list(EXTRA_ARGS)
# COM1 = PTTY (utmctl attach). COM2 = TCP client -> host MCP bridge.
# Use UTM's Serial device (not AdditionalArguments -unix): TcpClient is a
# first-class mode and is allowed through the sandbox.
#
# COM2 is wired unconditionally. It used to be gated behind UTM_BRIDGE=1, which
# meant a plain `make utm` silently dropped the port and the guest reported
# "bridge offline" forever — a config trap that reads as a bridge bug. A TCP
# client with nothing listening is harmless: the guest just sees it offline,
# which is the truth. Set UTM_BRIDGE=0 to leave the port out entirely.
serial = [{"Mode": "Ptty", "Target": "Auto"}]
if os.environ.get("UTM_BRIDGE", "1") != "0":
    addr = os.environ.get("OS_MCP_BRIDGE_ADDR", "127.0.0.1:7420")
    host, _, port = addr.rpartition(":")
    serial.append({
        "Mode": "TcpClient",
        "Target": "Auto",
        "TcpHostAddress": host or "127.0.0.1",
        "TcpPort": int(port or "7420"),
    })
cfg["Serial"] = serial
# The PC speaker needs an emulated sound card to reach the host. UTM creates
# VMs with Sound: [] and the chime is silent without this.
cfg["Sound"] = [{"Hardware": "intel-hda"}]
cfg.setdefault("System", {})["MemorySize"] = 1024
cfg["Display"] = [{
    "Hardware": "virtio-vga",
    "DynamicResolution": False,
    "NativeResolution": False,
    # Linear, not Nearest: the guest framebuffer is upscaled several times over
    # on a Retina display, and nearest-neighbour turns every anti-aliased glyph
    # back into blocks — undoing the whole point of the font atlas.
    "UpscalingFilter": "Linear",
    "DownscalingFilter": "Linear",
}]
p.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
print("bundled", Path(os.environ["UTM_DIR"]) / "Data" / "os.iso")
if os.environ.get("UTM_BRIDGE", "0") == "1":
    print("com2 TcpClient ->", os.environ.get("OS_MCP_BRIDGE_ADDR", "127.0.0.1:7420"))
PY

# Reload so UTM picks up ImageName (in-memory config would ignore our plist edit).
utm_quit
utm_open

if [[ "$START" == "1" ]]; then
  osascript -e "tell application \"UTM\" to start virtual machine named \"$VM_NAME\""
fi

echo "utm ok: $(du -h "$UTM_DIR/Data/os.iso" | awk '{print $1}') ISO in VM bundle"
if [[ "$BRIDGE" == "1" ]]; then
  echo ">>> COM2 TcpClient → MCP bridge at $BRIDGE_ADDR"
fi
echo ">>> Double-click 'os' in the sidebar to open the guest display window."
echo ">>> The black rectangle in the library list is only a thumbnail — not the GUI."
