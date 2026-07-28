#!/usr/bin/env bash
# Install the web-backed apps as real desktop applications.
#
# WhatsApp has no native Linux client at all — not on arm64, not on x86-64.
# What exists is web.whatsapp.com, and the difference between "a browser tab"
# and "an app" is entirely whether it has its own window, its own icon in the
# dock, and its own profile so a browser restart does not sign it out.
#
# Chromium's --app mode plus a .desktop file is that difference. Each app gets
# its own --user-data-dir so the WhatsApp session is not tied to the browsing
# profile, which is also what stops a "clear browsing data" from logging you
# out of your messages.
#
# --password-store=basic is there because Chromium otherwise asks the GNOME
# keyring to store credentials, and on a machine that logs in automatically the
# keyring has never been unlocked — so opening WhatsApp greets you with
# "Choose password for new keyring" instead of a QR code. These apps keep their
# session in site storage rather than the keyring, so nothing is lost by opting
# out, and the login keyring is left alone for everything else.
set -euo pipefail

PORT="${TEDDYOS_VM_SSH_PORT:-22}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
TARGET="$VM_USER@${TEDDYOS_VM_HOST:-localhost}"

# -n is not optional: this runs inside a `while read` loop, and without it ssh
# reads the loop's stdin and swallows every remaining app. The symptom is one
# app installed and no error — the loop simply ends early.
run() { ssh -n "${SSH_OPTS[@]}" "$TARGET" "$@"; }

ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET on port $PORT — boot the VM first" >&2
  exit 1
}

run 'command -v chromium >/dev/null' || {
  echo "error: chromium is not installed in the guest — run teddyos-provision.sh" >&2
  exit 1
}

# name|Display Name|url|categories|icon
#
# The icon name is not cosmetic. Every one of these launches chromium, so
# without a per-app icon the dock shows three identical Chromium marbles and
# the only way to tell WhatsApp from Gmail is to open it. The names below
# resolve in both WhiteSur and Papirus, so they survive a theme change.
APPS='whatsapp|WhatsApp|https://web.whatsapp.com|Network;InstantMessaging;|whatsapp
gmail|Gmail|https://mail.google.com|Network;Email;|gmail-desktop
teddysearch|teddysearch|https://teddysearch.com|Network;|system-search'

echo ">>> installing web apps"
while IFS='|' read -r id name url cats icon; do
  [[ -n "$id" ]] || continue
  echo "    $name  ->  $url   [$icon]"
  run "mkdir -p ~/.local/share/applications ~/.local/share/teddyos-apps/$id
cat > ~/.local/share/applications/teddyos-$id.desktop <<EOF
[Desktop Entry]
Type=Application
Name=$name
Comment=$name as a standalone window
Exec=chromium --ozone-platform-hint=auto --password-store=basic --app=$url --user-data-dir=\$HOME/.local/share/teddyos-apps/$id --class=teddyos-$id
Icon=$icon
Terminal=false
Categories=$cats
StartupWMClass=teddyos-$id
EOF"
done <<< "$APPS"

echo ">>> verifying each icon actually resolves"
# A .desktop pointing at a missing icon falls back to a generic tile silently.
while IFS='|' read -r id name url cats icon; do
  [[ -n "$id" ]] || continue
  found="$(run "find ~/.local/share/icons /usr/share/icons -name '$icon.svg' -o -name '$icon.png' 2>/dev/null | head -1" || true)"
  if [[ -z "$found" ]]; then
    echo "    WARNING: icon '$icon' for $name not found — it will render as a blank tile" >&2
  else
    echo "    $name: $icon ok"
  fi
done <<< "$APPS"

run 'update-desktop-database ~/.local/share/applications 2>/dev/null || true'

echo ">>> installed:"
run 'ls -1 ~/.local/share/applications/teddyos-*.desktop | sed "s|.*/|    |"'

cat <<'DONE'

>>> Each app keeps its own profile under ~/.local/share/teddyos-apps/.
    WhatsApp needs its QR scanned once from your phone; after that the
    session persists across reboots because the profile is on disk.
DONE
