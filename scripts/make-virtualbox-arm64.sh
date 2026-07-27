#!/bin/sh
set -eu

VM_NAME="${VBOX_VM_NAME:-teddyOS-arm64}"
FRONTEND="${VBOX_FRONTEND:-gui}"
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ISO="$ROOT/os-arm64.iso"
LOG="${VBOX_SERIAL_LOG:-/tmp/teddyos-arm64.log}"
BRIDGE_ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
BRIDGE_HOST="${BRIDGE_ADDR%%:*}"
BRIDGE_PORT="${BRIDGE_ADDR##*:}"

if ! command -v VBoxManage > /dev/null 2>&1; then
    echo "VirtualBox 7.2 or newer is required (VBoxManage was not found)." >&2
    exit 1
fi
if [ ! -f "$ISO" ]; then
    echo "ARM64 ISO is missing: $ISO" >&2
    echo "Run 'make iso-arm64' first." >&2
    exit 1
fi
case "$(uname -m)" in
    arm64|aarch64) ;;
    *)
        echo "This launcher is for an ARM64 host. Use 'make run' on x86-64." >&2
        exit 1
        ;;
esac

# ── Bridge ──────────────────────────────────────────────────────────────────
# The kernel talks to the host over COM2 (UART2). Start the bridge now so
# the port is open before the VM boots; otherwise the guest's first serial
# writes go nowhere and the UI shows "bridge offline".

bridge_up() {
    python3 -c \
      "import socket; s=socket.create_connection(('$BRIDGE_HOST', $BRIDGE_PORT), 0.5); s.close()" \
      > /dev/null 2>&1
}

if ! bridge_up; then
    BRIDGE_BIN="$ROOT/target/debug/os-mcp-bridge"
    if [ ! -x "$BRIDGE_BIN" ]; then
        echo "Building host bridge..."
        (cd "$ROOT" && PATH="/opt/homebrew/opt/rustup/bin:$PATH" \
            cargo build -p os-mcp-bridge 2>&1) || true
    fi
    if [ -x "$BRIDGE_BIN" ]; then
        echo "Starting host bridge on $BRIDGE_ADDR ..."
        BRIDGE_LOG="/tmp/teddyos-bridge.log"
        OS_MCP_BRIDGE_ADDR="$BRIDGE_ADDR" \
            nohup "$BRIDGE_BIN" >> "$BRIDGE_LOG" 2>&1 < /dev/null &
        # Wait up to 5 s for the port to open.
        i=0
        while [ "$i" -lt 50 ]; do
            bridge_up && break
            i=$((i + 1))
            sleep 0.1
        done
        if bridge_up; then
            echo "Bridge ready. Log: $BRIDGE_LOG"
        else
            echo "Warning: bridge did not come up in time — COM2 will be offline." >&2
        fi
    else
        echo "Warning: bridge binary not found — run 'make bridge' first." >&2
    fi
else
    echo "Bridge already running on $BRIDGE_ADDR."
fi

if ! VBoxManage showvminfo "$VM_NAME" > /dev/null 2>&1; then
    # VirtualBox spells this legacy CLI option `--ostype` (no second dash),
    # including current 7.2 ARM builds.
    VBoxManage createvm --name "$VM_NAME" --ostype Other_arm64 --register
    VBoxManage modifyvm "$VM_NAME" \
        --chipset armv8virtual \
        --firmware efi \
        --memory 2048 \
        --cpus 4 \
        --graphicscontroller qemuramfb \
        --vram 128 \
        --boot1 dvd \
        --boot2 none \
        --boot3 none \
        --boot4 none \
        --audio-enabled off \
        --nic1 nat
    VBoxManage storagectl "$VM_NAME" \
        --name SATA \
        --add sata \
        --controller IntelAhci
fi

state=$(VBoxManage showvminfo "$VM_NAME" --machinereadable |
    awk -F= '/^VMState=/{gsub(/"/, "", $2); print $2}')
if [ "$state" != "poweroff" ]; then
    echo "Stopping $VM_NAME to refresh its ISO..."
    # The GUI may already be closing while showvminfo still reports running.
    # Treat that race as success, then wait until settings are writable.
    VBoxManage controlvm "$VM_NAME" poweroff > /dev/null 2>&1 || true
    tries=0
    while [ "$tries" -lt 50 ]; do
        state=$(VBoxManage showvminfo "$VM_NAME" --machinereadable |
            awk -F= '/^VMState=/{gsub(/"/, "", $2); print $2}')
        case "$state" in
            poweroff|aborted) break ;;
        esac
        tries=$((tries + 1))
        sleep 0.1
    done
fi

# VirtualBox ARM exposes its built-in keyboard and tablet through OHCI. EHCI
# and xHCI move the devices behind controllers this kernel does not drive.
#
# `VMState=poweroff` can arrive just before the GUI releases its write lock.
# Retry that short handoff so a normal rebuild never randomly dies between
# stopping the old guest and configuring the new one.
modify_log=$(mktemp "${TMPDIR:-/tmp}/teddyos-vbox-modify.XXXXXX")
trap 'rm -f "$modify_log"' EXIT
tries=0
while ! VBoxManage modifyvm "$VM_NAME" \
    --chipset armv8virtual \
    --firmware efi \
    --memory 2048 \
    --cpus 4 \
    --graphicscontroller qemuramfb \
    --vram 128 \
    --usb-ohci on \
    --usb-ehci off \
    --usb-xhci off \
    --mouse usbtablet \
    --keyboard usb \
    --uart1 0x3f8 4 \
    --uart-mode1 file "$LOG" \
    --uart2 0x2f8 3 \
    --uart-mode2 tcpclient "$BRIDGE_HOST:$BRIDGE_PORT" \
    --audio-enabled off 2>"$modify_log"
do
    tries=$((tries + 1))
    if [ "$tries" -ge 50 ]; then
        cat "$modify_log" >&2
        exit 1
    fi
    sleep 0.1
done
VBOX_RES="${RESOLUTION:-1280x800}"
VBoxManage setextradata "$VM_NAME" \
    VBoxInternal2/EfiGraphicsResolution "$VBOX_RES"

# Detaching first matters: VirtualBox otherwise keeps serving the prior
# contents of a rebuilt ISO from its open medium.
VBoxManage storageattach "$VM_NAME" \
    --storagectl SATA --port 0 --device 0 --medium none 2>/dev/null || true
VBoxManage storageattach "$VM_NAME" \
    --storagectl SATA --port 0 --device 0 \
    --type dvddrive --medium "$ISO"

: >"$LOG"
VBoxManage startvm "$VM_NAME" --type "$FRONTEND"
echo "VirtualBox ARM64 ready: $VM_NAME"
echo "ISO: $ISO"
echo "Bridge: $BRIDGE_ADDR  (log: /tmp/teddyos-bridge.log)"
