#!/usr/bin/env bash
# Make the guest survive being moved between hypervisors.
#
# Two things in a Debian cloud image are pinned to the machine it first booted
# on, and both fail silently — the VM comes up, the desktop appears, and there
# is simply no network, which reads as a broken image rather than a config that
# was never portable:
#
#   1. cloud-init writes /etc/netplan/50-cloud-init.yaml matching the interface
#      by MAC address. UTM gives the NIC a different MAC than teddyos-vm.sh
#      does, so the match fails and no interface is configured.
#   2. cloud-init itself re-runs its datasource hunt on every boot. With no
#      seed drive attached (UTM does not carry one) that hunt has to time out
#      before boot continues.
#
# Run this once, after teddyos-provision.sh and before teddyos-utm.sh.
set -euo pipefail

PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
TARGET="$VM_USER@localhost"

run() { ssh "${SSH_OPTS[@]}" "$TARGET" "$@"; }

ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET on port $PORT — boot the VM first" >&2
  exit 1
}

echo ">>> hypervisor-independent networking"
# Match on the interface name pattern, not the MAC. Every hypervisor here
# presents a virtio NIC that udev names en*, whatever address it carries.
run 'sudo tee /etc/systemd/network/10-teddyos.network >/dev/null <<EOF
[Match]
Name=en*

[Network]
DHCP=yes
IPv6AcceptRA=yes
EOF
sudo systemctl enable --now systemd-networkd >/dev/null'

echo ">>> retiring the MAC-pinned netplan config"
run 'if [ -f /etc/netplan/50-cloud-init.yaml ]; then
       sudo mv /etc/netplan/50-cloud-init.yaml /etc/netplan/50-cloud-init.yaml.disabled
     fi'

echo ">>> disabling cloud-init (provisioning is done)"
run 'sudo touch /etc/cloud/cloud-init.disabled'

echo ">>> verifying the new config actually resolves"
# Assert, do not assume: the whole point of this script is a failure that is
# invisible until the next boot on a different host.
run 'sudo systemctl restart systemd-networkd
     for i in $(seq 1 20); do
       getent hosts deb.debian.org >/dev/null 2>&1 && { echo "    dns ok: $(ip -4 -o addr show scope global | awk "{print \$2, \$4}")"; exit 0; }
       sleep 1
     done
     echo "    ERROR: no name resolution after restarting networkd" >&2
     exit 1'

echo ">>> portable."
