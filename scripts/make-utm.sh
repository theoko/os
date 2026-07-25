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
UTM_DOCS="$HOME/Library/Containers/com.utmapp.UTM/Data/Documents"
UTM_DIR="$UTM_DOCS/${VM_NAME}.utm"
STAGED="$UTM_DOCS/Public/os-boot.iso"

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

osascript <<EOF
set isoPath to POSIX file "$STAGED"
tell application "UTM"
  activate
  try
    set old to virtual machine named "$VM_NAME"
    if status of old is not stopped then
      stop old by kill
      delay 1
    end if
    delete old
    delay 0.5
  end try
  make new virtual machine with properties {backend:qemu, configuration:{name:"$VM_NAME", architecture:"x86_64", memory:1024, hypervisor:false, uefi:true, displays:{{hardware:"virtio-vga"}}, drives:{{removable:true, source:isoPath}}}}
end tell
EOF

for _ in $(seq 1 20); do
  [[ -d "$UTM_DIR" ]] && break
  sleep 0.25
done
[[ -d "$UTM_DIR" ]] || { echo "error: VM bundle missing" >&2; exit 1; }

osascript <<EOF
tell application "UTM"
  set vm to virtual machine named "$VM_NAME"
  if status of vm is not stopped then
    stop vm by kill
    delay 1
  end if
end tell
EOF

mkdir -p "$UTM_DIR/Data"
cp -f "$STAGED" "$UTM_DIR/Data/os.iso"

UTM_DIR="$UTM_DIR" python3 <<'PY'
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
# USB 2.0 bus. UTM attaches `usb-tablet` to usb-bus.0, which enumerates at
# high speed (480 Mb/s) on ich9-usb-ehci1 — invisible to a UHCI-only guest driver.
cfg.setdefault("Input", {})
cfg["Input"]["UsbBusSupport"] = "2.0"
cfg["Input"]["UsbSharing"] = False
# `usb_version=1` makes the tablet advertise USB 1.1, so QEMU routes it to a
# full-speed (12 Mb/s) UHCI companion where the guest driver can bind it.
# Verified: `info usb` reports "Speed 12 Mb/s" and the guest logs "usb-tablet ready".
#
# AdditionalArguments MUST be a flat list of plain strings, one per argv token.
# A dict entry (e.g. {"ArgumentString": ...}) or a single "flag value" string
# fails to decode and UTM silently drops the VM from its library entirely.
EXTRA_ARGS = ["-global", "usb-tablet.usb_version=1"]
existing = cfg.setdefault("QEMU", {}).get("AdditionalArguments")
existing = [a for a in existing if isinstance(a, str)] if isinstance(existing, list) else []
for tok in EXTRA_ARGS:
    if tok not in existing:
        existing.append(tok)
cfg["QEMU"]["AdditionalArguments"] = existing
cfg.setdefault("System", {})["MemorySize"] = 1024
cfg["Display"] = [{
    "Hardware": "virtio-vga",
    "DynamicResolution": False,
    "NativeResolution": False,
    "UpscalingFilter": "Nearest",
    "DownscalingFilter": "Linear",
}]
p.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
print("bundled", Path(os.environ["UTM_DIR"]) / "Data" / "os.iso")
PY

# Reload so UTM picks up ImageName (in-memory config would ignore our plist edit).
osascript -e 'tell application "UTM" to quit' >/dev/null 2>&1 || true
sleep 2
open -a UTM
sleep 3

if [[ "$START" == "1" ]]; then
  osascript -e "tell application \"UTM\" to start virtual machine named \"$VM_NAME\""
fi

echo "utm ok: $(du -h "$UTM_DIR/Data/os.iso" | awk '{print $1}') ISO in VM bundle"
echo ">>> Double-click 'os' in the sidebar to open the guest display window."
echo ">>> The black rectangle in the library list is only a thumbnail — not the GUI."
