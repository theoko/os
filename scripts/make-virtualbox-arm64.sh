#!/bin/sh
set -eu

VM_NAME="${VBOX_VM_NAME:-teddyOS-arm64}"
FRONTEND="${VBOX_FRONTEND:-gui}"
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ISO="$ROOT/os-arm64.iso"
LOG="${VBOX_SERIAL_LOG:-/tmp/teddyos-arm64.log}"

if ! command -v VBoxManage >/dev/null 2>&1; then
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

if ! VBoxManage showvminfo "$VM_NAME" >/dev/null 2>&1; then
    VBoxManage createvm --name "$VM_NAME" --os-type Other_arm64 --register
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
    VBoxManage controlvm "$VM_NAME" poweroff >/dev/null 2>&1 || true
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
VBoxManage modifyvm "$VM_NAME" \
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
    --audio-enabled off
VBoxManage setextradata "$VM_NAME" \
    VBoxInternal2/EfiGraphicsResolution 1280x800

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
