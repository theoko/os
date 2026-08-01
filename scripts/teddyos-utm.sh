#!/usr/bin/env bash
# Hand the provisioned teddyOS disk to UTM (GPU path on Apple Silicon).
#
# Hard-won UTM 4.7.5 rules:
#   1. Create via AppleScript, then patch lightly — never invent a full config.
#   2. Keep UTM's disk ImageName; overwrite that file with our qcow2.
#   3. Never delete efi_vars.fd from Data/.
#   4. UsbBusSupport must be "3.0" (not "Usb3_0") or UTM drops the VM silently.
#   5. Ghost registry rows (extra UUIDs → same .utm path) break start-by-name.
#      Scrub them from Preferences only — never `utmctl delete` a ghost that
#      shares the package path (that deletes the live disk package).
#   6. Start via AppleScript so the SPICE window opens. `utmctl start` alone
#      leaves QEMU with -S (CPU frozen) until a display attaches → black forever.
#
#   TEDDYOS_UTM_START=1 ./scripts/teddyos-utm.sh
#   make desktop
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WORK="${TEDDYOS_VM_WORK:-$ROOT/.teddyos-vm}"
SRC_DISK="$WORK/disk.qcow2"
VM_NAME="${TEDDYOS_UTM_NAME:-teddyos}"
MEM="${TEDDYOS_VM_MEM:-8192}"
CPUS="${TEDDYOS_VM_CPUS:-4}"
DISPLAY_HW="${TEDDYOS_UTM_DISPLAY:-virtio-ramfb-gl}"
START="${TEDDYOS_UTM_START:-0}"
FORCE_COPY="${TEDDYOS_UTM_FORCE_COPY:-0}"

UTM_DOCS="$HOME/Library/Containers/com.utmapp.UTM/Data/Documents"
UTM_DIR="$UTM_DOCS/${VM_NAME}.utm"
UTM_PREF="$HOME/Library/Containers/com.utmapp.UTM/Data/Library/Preferences/com.utmapp.UTM.plist"

if [[ ! "$VM_NAME" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "error: bad TEDDYOS_UTM_NAME: $VM_NAME" >&2; exit 1
fi
[[ -d /Applications/UTM.app ]] || { echo "error: install UTM (brew install --cask utm)" >&2; exit 1; }
[[ -f "$SRC_DISK" ]] || {
  echo "error: $SRC_DISK missing — run: make linux-init && make linux-provision" >&2
  exit 1
}

utm_quit() {
  osascript -e 'tell application "UTM" to quit' >/dev/null 2>&1 || true
  for _ in $(seq 1 40); do pgrep -x UTM >/dev/null 2>&1 || return 0; sleep 0.25; done
}
utm_open() {
  open -a UTM
  for _ in $(seq 1 60); do
    osascript -e 'tell application "UTM" to count virtual machines' >/dev/null 2>&1 && return 0
    sleep 0.25
  done
  echo "error: UTM not scriptable" >&2; exit 1
}

bundle_uuid() {
  plutil -extract Information.UUID raw "$UTM_DIR/config.plist" 2>/dev/null || true
}

have_bundle() { [[ -f "$UTM_DIR/config.plist" ]]; }

# Headless brew-qemu must not hold the source disk.
headless_holds_disk() {
  local pid
  for pid in $(pgrep -f "qemu-system-aarch64" 2>/dev/null || true); do
    if ps -p "$pid" -o args= 2>/dev/null | grep -qF "$SRC_DISK"; then
      return 0
    fi
  done
  return 1
}

if headless_holds_disk; then
  echo "error: headless qemu still has $SRC_DISK open — power it off first" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Registry hygiene: drop UUIDs whose package is missing OR that share our
# package path but are not the UUID inside config.plist. Preferences only.
# ---------------------------------------------------------------------------
scrub_registry() {
  local keep_uuid="${1:-}"
  # UTM must be quit so it does not rewrite prefs under us.
  utm_quit
  UTM_PREF="$UTM_PREF" UTM_DIR="$UTM_DIR" VM_NAME="$VM_NAME" KEEP_UUID="$keep_uuid" python3 <<'PY'
import os, plistlib
from pathlib import Path

pref = Path(os.environ["UTM_PREF"])
utm_dir = Path(os.environ["UTM_DIR"])
vm_name = os.environ["VM_NAME"]
keep = os.environ.get("KEEP_UUID") or ""
if not keep and (utm_dir / "config.plist").is_file():
    try:
        cfg0 = plistlib.loads((utm_dir / "config.plist").read_bytes())
        keep = (cfg0.get("Information") or {}).get("UUID") or ""
    except Exception:
        keep = ""

if not pref.is_file():
    raise SystemExit(0)

cfg = plistlib.loads(pref.read_bytes())
reg = dict(cfg.get("Registry") or {})
new_reg = {}
dropped = []
for uuid, entry in reg.items():
    pkg = entry.get("Package") or {}
    path = pkg.get("Path") or ""
    exists = Path(path).joinpath("config.plist").is_file() if path else False
    same_path = path and Path(path).resolve() == utm_dir.resolve() if path and utm_dir.exists() else (
        path.endswith(f"/{vm_name}.utm") or path.endswith(f"{vm_name}.utm")
    )
    name = entry.get("Name") or ""
    if not exists:
        dropped.append((uuid, "missing package"))
        continue
    if same_path and keep and uuid.upper() != keep.upper():
        dropped.append((uuid, f"ghost for same path (keep {keep})"))
        continue
    if name == vm_name and keep and uuid.upper() != keep.upper() and same_path:
        dropped.append((uuid, "duplicate name/path"))
        continue
    new_reg[uuid] = entry

cfg["Registry"] = new_reg
cfg["VMEntryList"] = [u for u in (cfg.get("VMEntryList") or []) if u in new_reg]
if keep and keep in new_reg:
    rest = [u for u in cfg["VMEntryList"] if u != keep]
    cfg["VMEntryList"] = [keep] + rest
pref.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
for u, why in dropped:
    print(f"    scrubbed ghost {u} ({why})")
if not dropped:
    print("    registry clean")
print(f"    keep uuid: {keep or '(none)'}")
PY
  utm_open
}

stop_named() {
  local id
  for id in $(utmctl list 2>/dev/null | awk -v n="$VM_NAME" 'NR>1 && $3==n {print $1}'); do
    utmctl stop "$id" >/dev/null 2>&1 || true
  done
  # Also stop via AppleScript in case utmctl misses
  osascript <<EOF >/dev/null 2>&1 || true
tell application "UTM"
  try
    set vm to virtual machine named "$VM_NAME"
    if status of vm is not stopped then
      stop vm by kill
      delay 2
    end if
  end try
end tell
EOF
  sleep 1
}

# Full recreate: stop, scrub registry (no utmctl delete), remove package, create.
recreate_vm() {
  echo ">>> recreating UTM VM '$VM_NAME'"
  stop_named
  utm_quit
  # Wipe package only after UTM quit and all QEMU gone
  sleep 1
  for pid in $(pgrep -f "qemu-aarch64-softmmu" 2>/dev/null || true); do
    if ps -p "$pid" -o args= 2>/dev/null | grep -qF "$VM_NAME"; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  sleep 1
  # Scrub ALL registry rows for this name/path
  UTM_PREF="$UTM_PREF" UTM_DIR="$UTM_DIR" VM_NAME="$VM_NAME" python3 <<'PY'
import plistlib
from pathlib import Path
import os
pref = Path(os.environ["UTM_PREF"])
utm_dir = Path(os.environ["UTM_DIR"])
vm_name = os.environ["VM_NAME"]
if not pref.is_file():
    raise SystemExit(0)
cfg = plistlib.loads(pref.read_bytes())
reg = dict(cfg.get("Registry") or {})
new = {}
for uuid, entry in reg.items():
    path = (entry.get("Package") or {}).get("Path") or ""
    name = entry.get("Name") or ""
    if name == vm_name or path.endswith(f"/{vm_name}.utm") or path.endswith(f"{vm_name}.utm"):
        print(f"    drop registry {uuid} ({name})")
        continue
    new[uuid] = entry
cfg["Registry"] = new
cfg["VMEntryList"] = [u for u in (cfg.get("VMEntryList") or []) if u in new]
pref.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
PY
  rm -rf "$UTM_DIR"
  utm_open
  osascript <<EOF
tell application "UTM"
  activate
  make new virtual machine with properties {backend:qemu, configuration:{name:"$VM_NAME", architecture:"aarch64", memory:$MEM, hypervisor:true, uefi:true}}
end tell
EOF
  for _ in $(seq 1 40); do have_bundle && break; sleep 0.25; done
  have_bundle || { echo "error: UTM create failed — no $UTM_DIR/config.plist" >&2; exit 1; }
  echo "    created $(bundle_uuid)"
}

# ---------------------------------------------------------------------------
utm_open

need_create=0
if ! have_bundle; then
  need_create=1
elif [[ "$FORCE_COPY" == "1" ]]; then
  need_create=1
fi

if [[ "$need_create" == "1" ]]; then
  recreate_vm
else
  # Heal ghosts before we touch anything
  scrub_registry "$(bundle_uuid)"
fi

stop_named

# Decide whether to reinstall the guest disk image into the bundle.
DEST_NAME="$(python3 - "$UTM_DIR" <<'PY'
import plistlib, sys
from pathlib import Path
cfg = plistlib.loads((Path(sys.argv[1]) / "config.plist").read_bytes())
for d in cfg.get("Drive") or []:
    if d.get("ImageType") == "Disk" and d.get("ImageName"):
        print(d["ImageName"]); break
else:
    print("")
PY
)"
if [[ -z "$DEST_NAME" ]]; then
  DEST_NAME="disk.qcow2"
fi
DEST_DISK="$UTM_DIR/Data/$DEST_NAME"

COPY_DISK=0
if [[ ! -f "$DEST_DISK" ]]; then
  COPY_DISK=1
elif [[ "$FORCE_COPY" == "1" ]]; then
  COPY_DISK=1
elif [[ "$SRC_DISK" -nt "$DEST_DISK" ]]; then
  COPY_DISK=1
  echo ">>> source disk newer — refreshing UTM disk"
else
  echo ">>> keeping existing UTM disk ($DEST_NAME)"
fi

# Patch config + optional disk install
UTM_DIR="$UTM_DIR" SRC_DISK="$SRC_DISK" DEST_NAME="$DEST_NAME" \
MEM="$MEM" CPUS="$CPUS" DISPLAY_HW="$DISPLAY_HW" COPY_DISK="$COPY_DISK" python3 <<'PY'
import os, plistlib, shutil, uuid
from pathlib import Path

utm_dir = Path(os.environ["UTM_DIR"])
src = Path(os.environ["SRC_DISK"])
dest_name = os.environ["DEST_NAME"]
mem, cpus = int(os.environ["MEM"]), int(os.environ["CPUS"])
display_hw = os.environ["DISPLAY_HW"]
do_copy = os.environ["COPY_DISK"] == "1"

cfg_path = utm_dir / "config.plist"
data = utm_dir / "Data"
data.mkdir(exist_ok=True)
cfg = plistlib.loads(cfg_path.read_bytes())

# Single VirtIO disk; preserve ImageName so we don't orphan efi_vars pairing.
disk = {
    "Identifier": str(uuid.uuid4()).upper(),
    "ImageType": "Disk",
    "ImageName": dest_name,
    "Interface": "VirtIO",
    "InterfaceVersion": 1,
    "ReadOnly": False,
}
# Prefer existing identifier if same name
for d in cfg.get("Drive") or []:
    if d.get("ImageType") == "Disk" and d.get("ImageName") == dest_name and d.get("Identifier"):
        disk["Identifier"] = d["Identifier"]
        break
cfg["Drive"] = [disk]

if do_copy:
    print(f"    installing disk → Data/{dest_name} ({src.stat().st_size // (1024**2)} MB)")
    # Remove other qcow2 only (never efi_vars)
    for f in data.glob("*.qcow2"):
        if f.name != dest_name:
            f.unlink()
    shutil.copy2(src, data / dest_name)
else:
    print(f"    disk already present: Data/{dest_name}")

system = cfg.setdefault("System", {})
system["MemorySize"] = mem
system["CPUCount"] = cpus
system["Architecture"] = "aarch64"
system["Target"] = "virt"

qemu = cfg.setdefault("QEMU", {})
qemu["UEFIBoot"] = True
qemu["Hypervisor"] = True
qemu["RNGDevice"] = True
qemu["BalloonDevice"] = True

cfg["Display"] = [{
    "Hardware": display_hw,
    "DynamicResolution": True,
    "NativeResolution": True,
    "UpscalingFilter": "Linear",
    "DownscalingFilter": "Linear",
}]

nets = cfg.get("Network") or []
if nets:
    nets[0].setdefault("IsolateFromHost", False)
    nets[0].setdefault("Mode", "Shared")
    nets[0].setdefault("Hardware", "virtio-net-pci")
    cfg["Network"] = nets

cfg.setdefault("Input", {})["UsbBusSupport"] = "3.0"
if not cfg.get("Sound"):
    cfg["Sound"] = [{"Hardware": "intel-hda"}]
if not cfg.get("Serial"):
    cfg["Serial"] = [{"Mode": "Ptty", "Target": "Auto"}]

cfg_path.write_bytes(plistlib.dumps(cfg, fmt=plistlib.FMT_XML))
info = cfg.get("Information") or {}
print(f"    name     {info.get('Name')}")
print(f"    uuid     {info.get('UUID')}")
print(f"    display  {display_hw}")
print(f"    cpu/mem  {cpus} / {mem // 1024}G HVF")
print("    Data/:")
for f in sorted(data.iterdir()):
    print(f"      {f.name}  {f.stat().st_size}")
# Fail closed if efi_vars missing (UEFI will not boot)
if not any(p.name.startswith("efi") or "vars" in p.name for p in data.iterdir()):
    raise SystemExit("error: efi_vars.fd missing from Data/ — recreate the VM")
if not (data / dest_name).is_file():
    raise SystemExit(f"error: disk {dest_name} missing after install")
PY

# Reload so UTM reads patched plist; scrub ghosts to this UUID only.
UUID="$(bundle_uuid)"
scrub_registry "$UUID"
UUID="$(bundle_uuid)"
[[ -n "$UUID" ]] || { echo "error: no UUID in config.plist" >&2; exit 1; }

if ! utmctl list 2>/dev/null | awk -v u="$UUID" 'NR>1 && toupper($1)==toupper(u) {found=1} END{exit !found}'; then
  echo "error: UTM does not list $UUID after reload — config rejected" >&2
  utmctl list 2>&1 || true
  ls -la "$UTM_DIR" "$UTM_DIR/Data" 2>&1 || true
  exit 1
fi

if [[ "$START" == "1" ]]; then
  echo ">>> starting $VM_NAME ($UUID) with display window"
  # AppleScript start opens the SPICE window (unfreezes QEMU -S).
  # utmctl start alone often leaves the guest frozen with no window.
  osascript <<EOF
tell application "UTM"
  activate
  set vm to virtual machine named "$VM_NAME"
  if status of vm is stopped then
    start vm
  end if
end tell
EOF
  sleep 4
  status="$(utmctl list 2>/dev/null | awk -v u="$UUID" 'NR>1 && toupper($1)==toupper(u) {print $2; exit}')"
  echo ">>> status: ${status:-unknown}"
  if [[ "$status" != "started" ]]; then
    # One more try via utmctl then AppleScript name
    utmctl start "$UUID" 2>/dev/null || true
    sleep 2
    osascript -e "tell application \"UTM\" to start virtual machine named \"$VM_NAME\"" 2>/dev/null || true
    sleep 3
    status="$(utmctl list 2>/dev/null | awk -v u="$UUID" 'NR>1 && toupper($1)==toupper(u) {print $2; exit}')"
  fi
  if [[ "$status" != "started" ]]; then
    echo "error: VM did not stay running (status=${status:-none})" >&2
    utmctl list 2>&1 || true
    exit 1
  fi
  # Package must still exist after start (detect unlinked-disk failure early)
  if [[ ! -f "$DEST_DISK" ]]; then
    echo "error: disk package vanished after start — abort" >&2
    exit 1
  fi
fi

osascript -e 'tell application "UTM" to activate' >/dev/null 2>&1 || true

cat <<DONE

>>> '$VM_NAME' ready (uuid $UUID).
    Login:  teddy / teddyos
    Open the guest window if you only see the library list (double-click teddyos).
    Full screen: green button or ctrl-cmd-F
    First GNOME boot can take 30–90 seconds.
DONE
