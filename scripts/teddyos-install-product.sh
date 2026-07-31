#!/usr/bin/env bash
# Install the full teddyOS product into a running Debian guest.
#
# Why this exists: `teddyos-provision.sh` only installs GNOME + Chromium
# (substrate). The recording / live ISO experience — Search, welcome tour,
# WhiteSur look, dock, agents — lives under linux/ and was never applied to
# the UTM daily-driver disk. This script is that missing step.
#
#   # headless qemu (port-forward):
#   ./scripts/teddyos-install-product.sh
#
#   # UTM shared networking (guest has its own IP):
#   TEDDYOS_VM_HOST=192.168.64.13 TEDDYOS_VM_SSH_PORT=22 ./scripts/teddyos-install-product.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT="${TEDDYOS_VM_SSH_PORT:-22}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
HOST="${TEDDYOS_VM_HOST:-}"

# Auto-discover UTM guest from dhcpd_leases when host not set.
if [[ -z "$HOST" ]]; then
  if [[ "${TEDDYOS_VM_SSH_PORT:-}" == "2222" ]] || ss -ltn 2>/dev/null | grep -q ':2222'; then
    # Prefer classic headless forward if something listens on 2222
    if ssh -p 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR -o ConnectTimeout=2 -o BatchMode=yes \
        "${VM_USER}@localhost" true 2>/dev/null; then
      HOST=localhost
      PORT=2222
    fi
  fi
fi
if [[ -z "$HOST" ]]; then
  # Newest teddyos lease on the vmnet bridge
  HOST="$(awk '
    /^name=teddyos$/ { n=1; next }
    n && /^ip_address=/ { sub(/^ip_address=/,""); print; exit }
  ' /var/db/dhcpd_leases 2>/dev/null || true)"
  PORT=22
fi
HOST="${HOST:-localhost}"
if [[ "$HOST" == "localhost" && "$PORT" == "22" ]]; then
  # Fall back to headless forward
  PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
fi

SSH_BASE=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=8)
TARGET="${VM_USER}@${HOST}"
run() { ssh -p "$PORT" "${SSH_BASE[@]}" "$TARGET" "$@"; }
# scp uses -P (capital) for port; -p means preserve times.
copy() { scp -P "$PORT" "${SSH_BASE[@]}" "$@"; }

echo ">>> target $TARGET (port $PORT)"
ssh -p "$PORT" "${SSH_BASE[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET" >&2
  echo "       for UTM: TEDDYOS_VM_HOST=<guest-ip> TEDDYOS_VM_SSH_PORT=22 $0" >&2
  echo "       guest IP: cat /var/db/dhcpd_leases | grep -A2 teddyos" >&2
  exit 1
}

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/teddyos-product.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

echo ">>> staging product files"
mkdir -p "$STAGE"/{usr/bin,usr/lib/teddyos,usr/share/applications,usr/share/icons/hicolor/scalable/apps,usr/share/gnome-shell/extensions/teddyos@teddysearch.com,etc/dconf/db/local.d,etc/dconf/profile,etc/xdg/autostart,etc/chromium/policies/managed}

put() {
  local src="$1" dest="$STAGE/$2" mode="${3:-644}"
  [[ -f "$src" ]] || { echo "error: missing $src" >&2; exit 1; }
  mkdir -p "$(dirname "$dest")"
  cp "$src" "$dest"
  chmod "$mode" "$dest"
}

# --- binaries (same set as live ISO) ---
for b in teddyos-search teddyos-search-app; do
  put "linux/teddyos-search/$b" "usr/bin/$b" 755
done
put linux/teddyos-setup/teddyos-setup   usr/bin/teddyos-setup 755
put linux/teddyos-setup/teddyos-welcome usr/bin/teddyos-welcome 755
put linux/teddyos-claude/teddyos-claude usr/bin/teddyos-claude 755
put linux/teddyos-update/teddyos-update usr/bin/teddyos-update 755
for a in teddyos-agent teddyos-ask-all teddyos-perplexity teddyos-devin \
         teddyos-replit teddyos-accounts teddyos-open-signin \
         teddyos-linkedin teddyos-whatsapp teddyos-gmail teddyos-display; do
  put "linux/teddyos-agent/$a" "usr/bin/$a" 755
done
put linux/logging/teddyos-log-collect usr/bin/teddyos-log-collect 755

# Import path rewrites (match ISO / update payload)
if [[ "$(uname -s)" == Darwin ]]; then
  SED_INPLACE=(-i '')
else
  SED_INPLACE=(-i)
fi
sed "${SED_INPLACE[@]}" 's|sys.path.insert(0, str(Path(__file__).resolve().parent))|sys.path.insert(0, "/usr/lib/teddyos")|' \
  "$STAGE/usr/bin/teddyos-search" 2>/dev/null || true
# Drop local path inserts that break once files live in /usr/bin
for f in teddyos-search-app teddyos-setup teddyos-welcome; do
  [[ -f "$STAGE/usr/bin/$f" ]] || continue
  # Portable: remove lines that inject the repo-relative lib path
  python3 - "$STAGE/usr/bin/$f" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
t = p.read_text()
t2 = re.sub(r'^.*sys\.path\.insert\(0,.*resolve\(\)\.parent.*\n', '', t, flags=re.M)
t2 = re.sub(r'^.*teddyos-search["\']\)\).*\n', '', t2, flags=re.M)
if t2 != t:
    p.write_text(t2)
PY
done

# --- libraries ---
for m in caps search sandbox work_tools git_projects accounts audience intent \
         pending_ask progress; do
  [[ -f "linux/teddyos-search/$m.py" ]] && put "linux/teddyos-search/$m.py" "usr/lib/teddyos/$m.py"
done
put linux/logging/logutil.py usr/lib/teddyos/logutil.py

# --- shell extension + icons ---
put linux/shell-extension/teddyos@teddysearch.com/extension.js \
  "usr/share/gnome-shell/extensions/teddyos@teddysearch.com/extension.js"
put linux/shell-extension/teddyos@teddysearch.com/metadata.json \
  "usr/share/gnome-shell/extensions/teddyos@teddysearch.com/metadata.json"
for svg in linux/icons/*.svg; do
  [[ -e "$svg" ]] || continue
  case "$(basename "$svg")" in teddyos-logo.svg) continue ;; esac
  put "$svg" "usr/share/icons/hicolor/scalable/apps/$(basename "$svg")"
done

# --- desktop entries (product dock tiles) ---
cat > "$STAGE/usr/share/applications/teddyos-search.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Search
Comment=Ask teddyOS
Exec=teddyos-search-app
Icon=teddyos-search
Terminal=false
Categories=Utility;Core;
StartupNotify=true
EOF

cat > "$STAGE/usr/share/applications/teddyos-web.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Web
Comment=Browse the web
Exec=chromium --password-store=basic
Icon=teddyos-web
Terminal=false
Categories=Network;WebBrowser;
StartupNotify=true
EOF

cat > "$STAGE/usr/share/applications/teddyos-whatsapp.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=WhatsApp
Comment=Set up WhatsApp on teddyOS
Exec=teddyos-whatsapp
Icon=teddyos-whatsapp
Terminal=false
Categories=Network;InstantMessaging;
StartupNotify=true
EOF

cat > "$STAGE/usr/share/applications/teddyos-welcome.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Welcome
Comment=A short tour of teddyOS
Exec=teddyos-welcome
Icon=teddyos-search
Terminal=false
Categories=Utility;
NoDisplay=true
EOF

cat > "$STAGE/etc/xdg/autostart/teddyos-welcome.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Welcome
Exec=teddyos-welcome --if-needed
Icon=teddyos-search
Hidden=false
X-GNOME-Autostart-enabled=true
OnlyShowIn=GNOME;
EOF

cat > "$STAGE/etc/xdg/autostart/teddyos-display.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=teddyOS Display
Exec=teddyos-display
Hidden=false
X-GNOME-Autostart-enabled=true
OnlyShowIn=GNOME;
EOF

# Kill stock Debian tour; product welcome is teddyos-welcome
cat > "$STAGE/etc/xdg/autostart/org.gnome.Tour.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Tour
Exec=true
Hidden=true
NoDisplay=true
X-GNOME-Autostart-enabled=false
EOF

# dconf defaults (dock, theme names, no welcome dialog)
mkdir -p "$STAGE/etc/dconf/profile"
printf '%s\n' 'user-db:user' 'system-db:local' > "$STAGE/etc/dconf/profile/user"
./linux/iso/render-dconf.sh > "$STAGE/etc/dconf/db/local.d/00-teddyos"

if [[ -f linux/iso/chromium-policy.json ]]; then
  put linux/iso/chromium-policy.json etc/chromium/policies/managed/teddyos.json
fi

# software stamp
mkdir -p "$STAGE/etc"
cat > "$STAGE/etc/teddyos-software" <<EOF
version=$(cat VERSION 2>/dev/null || echo 0.0.0)
commit=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
EOF

echo ">>> packing and uploading"
# Archive must live outside STAGE or bsdtar refuses ("Can't add archive to itself").
# Files only — directory entries in the tar would chmod live paths like /usr/bin
# (macOS mkdir modes applied on extract), which removes execute for others and
# bricks the guest (sudo: Permission denied, PATH commands vanish).
PRODUCT_TGZ="$(mktemp "${TMPDIR:-/tmp}/teddyos-product.XXXXXX.tgz")"
trap 'rm -rf "$STAGE" "$PRODUCT_TGZ"' EXIT
(
  cd "$STAGE"
  COPYFILE_DISABLE=1 find . -type f -print0 \
    | COPYFILE_DISABLE=1 tar -czf "$PRODUCT_TGZ" --null -T -
)
copy "$PRODUCT_TGZ" "$TARGET:/tmp/teddyos-product.tgz"

echo ">>> installing on guest (needs sudo)"
# --no-overwrite-dir / --no-same-permissions: never stomp host dir modes.
run 'set -e
     sudo tar -xzf /tmp/teddyos-product.tgz -C / --no-same-owner --no-same-permissions --no-overwrite-dir
     sudo chmod 755 /usr/bin/teddyos-* /usr/bin 2>/dev/null || true
     sudo chmod 755 /usr /usr/share /usr/lib /usr/lib/teddyos 2>/dev/null || true
     sudo gtk-update-icon-cache -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
     sudo update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
     sudo dconf update >/dev/null 2>&1 || true
     sudo DEBIAN_FRONTEND=noninteractive apt-get remove -y -qq gnome-tour >/dev/null 2>&1 || true
     rm -f /tmp/teddyos-product.tgz
     command -v teddyos-search-app
     command -v teddyos-welcome
     test -f /usr/share/applications/teddyos-search.desktop
     echo installed-ok'

echo ">>> look (WhiteSur theme, dock, wallpaper)"
# teddyos-look expects TEDDYOS_VM_HOST / PORT
export TEDDYOS_VM_HOST="$HOST"
export TEDDYOS_VM_SSH_PORT="$PORT"
export TEDDYOS_VM_USER="$VM_USER"
if [[ -x "$ROOT/scripts/teddyos-look.sh" ]]; then
  # look script defaults PORT=22 which matches UTM; for 2222 it must be set
  bash "$ROOT/scripts/teddyos-look.sh" 2>&1 || {
    echo "warning: teddyos-look.sh failed — product binaries are installed; theme may be stock" >&2
  }
else
  echo "warning: teddyos-look.sh missing" >&2
fi

echo ">>> web apps (WhatsApp tile profile)"
if [[ -x "$ROOT/scripts/teddyos-apps.sh" ]]; then
  bash "$ROOT/scripts/teddyos-apps.sh" 2>&1 || true
fi

echo ">>> reload user session hints"
run 'gsettings set org.gnome.shell welcome-dialog-last-shown-version "99.0" 2>/dev/null || true
     # Favorites dock (in case dconf system-db not picked up until next login)
     gsettings set org.gnome.shell favorite-apps "[\"teddyos-search.desktop\", \"teddyos-web.desktop\", \"teddyos-whatsapp.desktop\", \"org.gnome.Nautilus.desktop\"]" 2>/dev/null || true
     true'

cat <<DONE

>>> product installed on $TARGET.

  Log out and back in (or reboot the UTM guest) so GNOME reloads the shell
  extension, theme, and dock.

  You should then see:
    - Monterey-style wallpaper + WhiteSur look (if look succeeded)
    - Dock: Search · Web · WhatsApp · Files
    - teddyOS Welcome tour (not stock Debian Tour)
    - Search: "i wanna work on …" / Get help

  Reboot from UTM: guest menu → Restart, or:
    ssh $TARGET 'sudo reboot'
DONE
