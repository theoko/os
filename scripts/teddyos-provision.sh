#!/usr/bin/env bash
# Turn the bare Debian substrate into something that can be lived in.
#
# Runs over ssh against the VM that scripts/teddyos-vm.sh boots. Idempotent:
# apt and npm both no-op on a second run, so this is safe to re-run after a
# partial failure rather than something you get one shot at.
#
# What it installs is exactly the day-one bar, nothing speculative:
#   desktop  — GNOME on Wayland
#   browser  — Chromium (Google ships no Chrome for Linux/arm64; see below)
#   agent    — Claude Code
#   chat     — WhatsApp Web as an installed web app
set -euo pipefail

PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
TARGET="$VM_USER@localhost"

run() { ssh "${SSH_OPTS[@]}" "$TARGET" "$@"; }

ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET on port $PORT" >&2
  echo "       boot it first: ./scripts/teddyos-vm.sh --headless" >&2
  exit 1
}

echo ">>> apt update"
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq'

echo ">>> desktop (GNOME on Wayland) — this is the long one"
# gnome-core rather than the full gnome task: the task pulls games, and a
# daily driver's first impression should not be that it came with solitaire.
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
       gnome-core gdm3 \
       fonts-noto fonts-noto-color-emoji \
       pipewire-audio wireplumber \
       xdg-utils desktop-file-utils \
       qrencode'

echo ">>> browser"
# Google publishes Chrome for Linux on amd64 only. On aarch64 the browser is
# Chromium, which is the same engine minus Google's proprietary bits — the one
# that matters in practice is Widevine, so DRM video (Netflix et al.) will not
# play. If that is a dealbreaker, it is an argument for x86-64 hardware in
# route C, not something to fix here.
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq chromium'

echo ">>> tooling + Claude Code"
# gh: Search “work on owner/repo” and Connect GitHub need the GitHub CLI.
# ISO already ships it; daily-driver provision must match.
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
       nodejs npm git gh ripgrep build-essential python3 python3-venv'
run 'sudo npm install -g --silent @anthropic-ai/claude-code 2>&1 | tail -3'

echo ">>> boot to the desktop"
run 'sudo systemctl set-default graphical.target'

echo ">>> teddyOS branding (login + os-release, no Debian watermark)"
# Shell script on the host; uses same SSH as this provision run.
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TEDDYOS_VM_SSH_PORT="$PORT" TEDDYOS_VM_USER="$VM_USER" TEDDYOS_VM_HOST="${TEDDYOS_VM_HOST:-localhost}" \
  bash "$ROOT_DIR/scripts/teddyos-brand.sh" 2>&1 || {
    echo "    warning: brand step failed (non-fatal)" >&2
  }

echo ">>> no Debian/GNOME welcome tour"
# gnome-core pulls gnome-tour ("Welcome to Debian / Take the Tour"). First boot
# should land on the desktop, not a distro greeter.
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get remove -y -qq gnome-tour 2>/dev/null || true'
run 'sudo mkdir -p /etc/xdg/autostart /etc/dconf/db/local.d /etc/dconf/profile'
run "printf '%s\n' \
  '[Desktop Entry]' 'Type=Application' 'Name=Tour' 'Exec=true' \
  'Hidden=true' 'NoDisplay=true' 'X-GNOME-Autostart-enabled=false' \
  | sudo tee /etc/xdg/autostart/org.gnome.Tour.desktop >/dev/null"
run "printf '%s\n' 'user-db:user' 'system-db:local' \
  | sudo tee /etc/dconf/profile/user >/dev/null"
run 'VER=$(gnome-shell --version 2>/dev/null | awk "{print \$3}"); VER=${VER:-48}
printf "%s\n" "[org/gnome/shell]" "welcome-dialog-last-shown-version='"'"'$VER'"'"'" \
  | sudo tee /etc/dconf/db/local.d/00-teddyos-no-welcome >/dev/null
sudo dconf update
gsettings set org.gnome.shell welcome-dialog-last-shown-version "$VER" 2>/dev/null || true
'

echo ">>> versions"
run 'echo "  chromium: $(chromium --version 2>/dev/null || echo MISSING)"
     echo "  node:     $(node --version 2>/dev/null || echo MISSING)"
     echo "  claude:   $(claude --version 2>/dev/null || echo MISSING)"
     echo "  gnome:    $(gnome-shell --version 2>/dev/null || echo MISSING)"
     echo "  default:  $(systemctl get-default)"'

cat <<'DONE'

>>> provisioned.

Next: reboot into the desktop under UTM (GPU-accelerated), not under
plain qemu — brew's qemu has no virgl, so GNOME would software-render.

    ./scripts/teddyos-utm.sh
DONE
