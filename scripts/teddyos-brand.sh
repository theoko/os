#!/usr/bin/env bash
# Strip Debian watermark/branding from a running teddyOS guest.
#
# Targets what people actually see:
#   - GDM login logo (bottom-left "Debian" mark) — removed
#   - vendor-logos alternative → teddyOS logos
#   - /etc/os-release, /etc/issue, lsb-release
#   - debian-logo pixmaps / plymouth watermark
#   - Debian homepage / reference desktop entries
#
# Keeps ID_LIKE=debian so apt and packages still behave.
#
#   TEDDYOS_VM_HOST=… TEDDYOS_VM_SSH_PORT=22 ./scripts/teddyos-brand.sh
#   (defaults: localhost:2222 for headless)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${TEDDYOS_VM_SSH_PORT:-2222}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
HOST="${TEDDYOS_VM_HOST:-localhost}"
VERSION="$(cat "$ROOT/VERSION" 2>/dev/null || echo 0.0.0)"
LOGO_SVG="${TEDDYOS_LOGO_SVG:-$ROOT/linux/icons/teddyos-logo.svg}"

SSH_BASE=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=8)
run() { ssh -p "$PORT" "${SSH_BASE[@]}" "${VM_USER}@${HOST}" "$@"; }
copy() { scp -P "$PORT" "${SSH_BASE[@]}" "$@"; }

ssh -p "$PORT" "${SSH_BASE[@]}" -o BatchMode=yes "${VM_USER}@${HOST}" true 2>/dev/null || {
  echo "error: cannot ssh to ${VM_USER}@${HOST}:$PORT" >&2
  exit 1
}

echo ">>> teddyOS brand  v$VERSION  →  ${VM_USER}@${HOST}:$PORT"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/teddyos-brand.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/logos"

if [[ -f "$LOGO_SVG" ]]; then
  cp "$LOGO_SVG" "$STAGE/teddyos-logo.svg"
fi

if [[ -f "$LOGO_SVG" ]] && command -v rsvg-convert >/dev/null 2>&1; then
  for s in 64 128 256; do
    rsvg-convert -w "$s" -h "$s" -o "$STAGE/logos/logo-${s}.png" "$LOGO_SVG"
    rsvg-convert -w "$s" -h "$s" -o "$STAGE/logos/logo-text-${s}.png" "$LOGO_SVG"
    rsvg-convert -w "$s" -h "$s" -o "$STAGE/logos/logo-text-version-${s}.png" "$LOGO_SVG"
  done
  cp "$LOGO_SVG" "$STAGE/logos/logo.svg"
  cp "$LOGO_SVG" "$STAGE/logos/logo-text.svg"
  cp "$LOGO_SVG" "$STAGE/logos/logo-text-version.svg"
  echo "    logos rendered on host from teddyos-logo.svg"
else
  echo "    note: host has no rsvg-convert — guest will render SVG if librsvg2-bin is installed"
fi

copy -r "$STAGE/logos" "${VM_USER}@${HOST}:/tmp/teddyos-brand-logos" 2>/dev/null || true
if [[ -f "$STAGE/teddyos-logo.svg" ]]; then
  copy "$STAGE/teddyos-logo.svg" "${VM_USER}@${HOST}:/tmp/teddyos-logo.svg" 2>/dev/null || true
fi

# Pass version as env; install logos + convert on guest when possible.
run "export VERSION='$VERSION'; bash -s" <<'REMOTE'
set -euo pipefail
VERSION="${VERSION:-0.0.0}"

echo ">>> os-release / issue / lsb"
# /etc/os-release is often a symlink to /usr/lib/os-release — write the real file.
OS_RELEASE_BODY=$(printf '%s\n' \
  "PRETTY_NAME=\"teddyOS ${VERSION}\"" \
  "NAME=\"teddyOS\"" \
  "VERSION_ID=\"${VERSION}\"" \
  "VERSION=\"${VERSION}\"" \
  "VERSION_CODENAME=teddy" \
  "ID=teddyos" \
  "ID_LIKE=debian" \
  "HOME_URL=\"https://teddysearch.com/\"" \
  "SUPPORT_URL=\"https://teddysearch.com/\"" \
  "BUG_REPORT_URL=\"https://github.com/theoko/os/issues\"")
printf '%s\n' "$OS_RELEASE_BODY" | sudo tee /usr/lib/os-release >/dev/null
# If /etc/os-release is not a symlink, keep it in sync; if it is, it already points here.
if [ -L /etc/os-release ]; then
  :
else
  printf '%s\n' "$OS_RELEASE_BODY" | sudo tee /etc/os-release >/dev/null
fi

sudo tee /etc/issue >/dev/null <<'EOF'
teddyOS \n \l

EOF
sudo tee /etc/issue.net >/dev/null <<'EOF'
teddyOS
EOF

# Always create lsb-release (guest may not have had one).
sudo tee /etc/lsb-release >/dev/null <<EOF
DISTRIB_ID=teddyOS
DISTRIB_RELEASE=$VERSION
DISTRIB_CODENAME=teddy
DISTRIB_DESCRIPTION="teddyOS $VERSION"
EOF

# Motd noise
sudo rm -f /etc/motd 2>/dev/null || true
sudo mkdir -p /etc/update-motd.d
sudo tee /etc/update-motd.d/00-teddyos >/dev/null <<'EOF'
#!/bin/sh
echo "teddyOS"
EOF
sudo chmod +x /etc/update-motd.d/00-teddyos
for f in /etc/update-motd.d/*; do
  [ -e "$f" ] || continue
  case "$(basename "$f")" in
    00-teddyos) ;;
    *) sudo chmod a-x "$f" 2>/dev/null || true ;;
  esac
done

echo ">>> logos (vendor + GDM + pixmaps)"
# Prefer librsvg on the guest for crisp PNGs.
if [ -f /tmp/teddyos-logo.svg ]; then
  if ! command -v rsvg-convert >/dev/null 2>&1; then
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq librsvg2-bin >/dev/null 2>&1 || true
  fi
fi

sudo mkdir -p /usr/share/desktop-base/teddyos-logos
if [ -f /tmp/teddyos-logo.svg ] && command -v rsvg-convert >/dev/null 2>&1; then
  for s in 64 128 256; do
    sudo rsvg-convert -w "$s" -h "$s" -o /usr/share/desktop-base/teddyos-logos/logo-${s}.png /tmp/teddyos-logo.svg
    sudo cp /usr/share/desktop-base/teddyos-logos/logo-${s}.png /usr/share/desktop-base/teddyos-logos/logo-text-${s}.png
    sudo cp /usr/share/desktop-base/teddyos-logos/logo-${s}.png /usr/share/desktop-base/teddyos-logos/logo-text-version-${s}.png
  done
  sudo cp /tmp/teddyos-logo.svg /usr/share/desktop-base/teddyos-logos/logo.svg
  sudo cp /tmp/teddyos-logo.svg /usr/share/desktop-base/teddyos-logos/logo-text.svg
  sudo cp /tmp/teddyos-logo.svg /usr/share/desktop-base/teddyos-logos/logo-text-version.svg
elif [ -d /tmp/teddyos-brand-logos ] && [ -f /tmp/teddyos-brand-logos/logo-64.png ]; then
  sudo cp -a /tmp/teddyos-brand-logos/. /usr/share/desktop-base/teddyos-logos/
fi

# Emblem slaves so update-alternatives doesn't leave empty vendor emblems
if [ -f /usr/share/desktop-base/teddyos-logos/logo-64.png ]; then
  for size in 64 128 256; do
    sudo mkdir -p "/usr/share/desktop-base/teddyos-logos/emblems/${size}x${size}"
    sudo cp "/usr/share/desktop-base/teddyos-logos/logo-${size}.png" \
      "/usr/share/desktop-base/teddyos-logos/emblems/${size}x${size}/emblem-vendor.png"
    sudo cp "/usr/share/desktop-base/teddyos-logos/logo-${size}.png" \
      "/usr/share/desktop-base/teddyos-logos/emblems/${size}x${size}/emblem-vendor-symbolic.png"
    sudo cp "/usr/share/desktop-base/teddyos-logos/logo-${size}.png" \
      "/usr/share/desktop-base/teddyos-logos/emblems/${size}x${size}/emblem-vendor-white.png"
  done
  sudo mkdir -p /usr/share/desktop-base/teddyos-logos/emblems/scalable
  if [ -f /usr/share/desktop-base/teddyos-logos/logo.svg ]; then
    sudo cp /usr/share/desktop-base/teddyos-logos/logo.svg \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor.svg
    sudo cp /usr/share/desktop-base/teddyos-logos/logo.svg \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor-symbolic.svg
    sudo cp /usr/share/desktop-base/teddyos-logos/logo.svg \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor-white.svg
  fi

  # Full alternative with slaves (matches desktop-base layout)
  sudo update-alternatives --install /usr/share/images/vendor-logos vendor-logos \
    /usr/share/desktop-base/teddyos-logos 100 \
    --slave /usr/share/icons/vendor/64x64/emblems/emblem-vendor.png emblem-vendor-64 \
      /usr/share/desktop-base/teddyos-logos/emblems/64x64/emblem-vendor.png \
    --slave /usr/share/icons/vendor/128x128/emblems/emblem-vendor.png emblem-vendor-128 \
      /usr/share/desktop-base/teddyos-logos/emblems/128x128/emblem-vendor.png \
    --slave /usr/share/icons/vendor/256x256/emblems/emblem-vendor.png emblem-vendor-256 \
      /usr/share/desktop-base/teddyos-logos/emblems/256x256/emblem-vendor.png \
    --slave /usr/share/icons/vendor/scalable/emblems/emblem-vendor.svg emblem-vendor-scalable \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor.svg \
    --slave /usr/share/icons/vendor/64x64/emblems/emblem-vendor-symbolic.png emblem-vendor-symbolic-64 \
      /usr/share/desktop-base/teddyos-logos/emblems/64x64/emblem-vendor-symbolic.png \
    --slave /usr/share/icons/vendor/128x128/emblems/emblem-vendor-symbolic.png emblem-vendor-symbolic-128 \
      /usr/share/desktop-base/teddyos-logos/emblems/128x128/emblem-vendor-symbolic.png \
    --slave /usr/share/icons/vendor/256x256/emblems/emblem-vendor-symbolic.png emblem-vendor-symbolic-256 \
      /usr/share/desktop-base/teddyos-logos/emblems/256x256/emblem-vendor-symbolic.png \
    --slave /usr/share/icons/vendor/scalable/emblems/emblem-vendor-symbolic.svg emblem-vendor-symbolic-scalable \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor-symbolic.svg \
    --slave /usr/share/icons/vendor/64x64/emblems/emblem-vendor-white.png emblem-vendor-white-64 \
      /usr/share/desktop-base/teddyos-logos/emblems/64x64/emblem-vendor-white.png \
    --slave /usr/share/icons/vendor/128x128/emblems/emblem-vendor-white.png emblem-vendor-white-128 \
      /usr/share/desktop-base/teddyos-logos/emblems/128x128/emblem-vendor-white.png \
    --slave /usr/share/icons/vendor/256x256/emblems/emblem-vendor-white.png emblem-vendor-white-256 \
      /usr/share/desktop-base/teddyos-logos/emblems/256x256/emblem-vendor-white.png \
    --slave /usr/share/icons/vendor/scalable/emblems/emblem-vendor-white.svg emblem-vendor-white-scalable \
      /usr/share/desktop-base/teddyos-logos/emblems/scalable/emblem-vendor-white.svg \
    2>/dev/null || true
  sudo update-alternatives --set vendor-logos /usr/share/desktop-base/teddyos-logos 2>/dev/null || true
fi

# Pixmaps / plymouth: replace common Debian swirl paths consumers hardcode.
if [ -f /tmp/teddyos-logo.svg ] && command -v rsvg-convert >/dev/null 2>&1; then
  sudo mkdir -p /usr/share/pixmaps /usr/share/icons/hicolor/scalable/apps
  sudo cp /tmp/teddyos-logo.svg /usr/share/icons/hicolor/scalable/apps/teddyos-logo.svg
  sudo rsvg-convert -w 48 -h 48 -o /usr/share/pixmaps/teddyos-logo.png /tmp/teddyos-logo.svg
  sudo cp /usr/share/pixmaps/teddyos-logo.png /usr/share/pixmaps/debian-logo.png 2>/dev/null || true
  # Plymouth watermark (boot splash + some themes)
  if [ -f /usr/share/plymouth/debian-logo.png ]; then
    sudo rsvg-convert -w 121 -h 150 -o /usr/share/plymouth/debian-logo.png /tmp/teddyos-logo.svg 2>/dev/null \
      || sudo cp /usr/share/pixmaps/teddyos-logo.png /usr/share/plymouth/debian-logo.png
  fi
  sudo gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true
elif [ -f /usr/share/desktop-base/teddyos-logos/logo-64.png ]; then
  sudo mkdir -p /usr/share/pixmaps
  sudo cp /usr/share/desktop-base/teddyos-logos/logo-64.png /usr/share/pixmaps/teddyos-logo.png
  sudo cp /usr/share/pixmaps/teddyos-logo.png /usr/share/pixmaps/debian-logo.png 2>/dev/null || true
  if [ -f /usr/share/plymouth/debian-logo.png ]; then
    sudo cp /usr/share/pixmaps/teddyos-logo.png /usr/share/plymouth/debian-logo.png
  fi
fi

echo ">>> GDM greeter — no Debian watermark"
# Empty logo removes the bottom-left vendor mark entirely (cleanest product login).
# dconf keyfile strings MUST use single quotes.
sudo mkdir -p /etc/gdm3
sudo tee /etc/gdm3/greeter.dconf-defaults >/dev/null <<'EOF'
# teddyOS greeter — no Debian vendor watermark
[org/gnome/login-screen]
logo=''
enable-smartcard-authentication=false
EOF

# Also system dconf (takes priority over package file-db defaults)
sudo mkdir -p /etc/dconf/db/gdm.d /etc/dconf/profile
if [ ! -f /etc/dconf/profile/gdm ]; then
  sudo tee /etc/dconf/profile/gdm >/dev/null <<'EOF'
user-db:user
system-db:gdm
file-db:/usr/share/gdm/greeter-dconf-defaults
EOF
fi
sudo tee /etc/dconf/db/gdm.d/00-teddyos-logo >/dev/null <<'EOF'
[org/gnome/login-screen]
logo=''
EOF
sudo dconf update 2>/dev/null || true

# Recompile the greeter dconf blob GDM actually loads.
# Debian links /usr/share/gdm/dconf/90-debian-settings → /etc/gdm3/greeter.dconf-defaults
if [ -d /usr/share/gdm/dconf ]; then
  sudo dconf compile /usr/share/gdm/greeter-dconf-defaults /usr/share/gdm/dconf 2>/dev/null || true
fi
# Some installs use generate-config
if [ -x /usr/share/gdm/generate-config ]; then
  sudo /usr/share/gdm/generate-config 2>/dev/null || true
fi
sudo glib-compile-schemas /usr/share/glib-2.0/schemas 2>/dev/null || true

# Hide Debian reference desktop entries
for f in /usr/share/desktop-base/debian-homepage.desktop \
         /usr/share/desktop-base/debian-reference.desktop \
         /usr/share/desktop-base/debian-security.desktop; do
  if [ -f "$f" ]; then
    sudo mkdir -p /usr/local/share/applications
    sudo tee "/usr/local/share/applications/$(basename "$f")" >/dev/null <<EOF
[Desktop Entry]
Type=Application
Name=Hidden
NoDisplay=true
Hidden=true
EOF
  fi
done

# Optional: avoid "Debian" in hostnamectl pretty name (already from os-release)
# Do not change kernel package names or apt sources — substrate stays Debian.

echo ">>> verify"
echo "    $(grep ^PRETTY_NAME= /usr/lib/os-release)"
echo "    issue: $(head -1 /etc/issue)"
echo "    lsb: $(grep DISTRIB_DESCRIPTION /etc/lsb-release 2>/dev/null || echo missing)"
echo "    vendor-logos -> $(readlink -f /usr/share/images/vendor-logos 2>/dev/null || echo '?')"
echo "    gdm greeter logo: $(grep -E '^logo=' /etc/gdm3/greeter.dconf-defaults 2>/dev/null || true)"
echo "    gdm.d logo: $(grep -E '^logo=' /etc/dconf/db/gdm.d/00-teddyos-logo 2>/dev/null || true)"
echo "done-brand"
REMOTE

echo
echo ">>> brand applied."
echo "    Login-screen watermark is cleared in config."
echo "    To see it on the login screen, log out or reboot the UTM guest"
echo "    (restarting gdm3 ends the current graphical session):"
echo "      ssh teddy@${HOST} 'sudo systemctl restart gdm3'"
echo "    or reboot from the UTM window."
