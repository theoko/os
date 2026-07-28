#!/usr/bin/env bash
# Make the desktop simple and beautiful, in the way macOS is.
#
# What that actually decomposes into, because "looks like macOS" is not a
# setting:
#
#   a dock          bottom, centred, hidden until wanted, icons that grow
#   a menu bar      thin, translucent, always there
#   window buttons  left, in close/min/max order
#   type            one humanist sans everywhere, generously sized
#   depth           blur behind the shell instead of flat panels
#   restraint       no desktop icons, no tray clutter, one wallpaper
#
# GNOME 48 already gets the last one right by default, which is why it is the
# base rather than something more configurable. The rest is this script.
#
# Settings land in /etc/dconf as *defaults*, not as writes to the user's
# database. Two reasons: dconf writes need a live session bus and there is no
# session while we are provisioning over ssh, and defaults leave every one of
# these overridable from Settings without fighting a script that reasserts them.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

PORT="${TEDDYOS_VM_SSH_PORT:-22}"
VM_USER="${TEDDYOS_VM_USER:-teddy}"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o LogLevel=ERROR -o ConnectTimeout=5)
TARGET="$VM_USER@${TEDDYOS_VM_HOST:-localhost}"

run() { ssh "${SSH_OPTS[@]}" "$TARGET" "$@"; }

ssh "${SSH_OPTS[@]}" -o BatchMode=yes "$TARGET" true 2>/dev/null || {
  echo "error: cannot ssh to $TARGET on port $PORT — boot the VM first" >&2
  exit 1
}

echo ">>> dock, blur, user themes"
run 'sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
       gnome-shell-extension-dashtodock \
       gnome-shell-extension-blur-my-shell \
       gnome-shell-extension-light-style \
       gnome-shell-extension-appindicator \
       gnome-shell-extensions \
       gnome-tweaks \
       papirus-icon-theme \
       fonts-inter \
       sassc libglib2.0-dev-bin'

# --- extensions Debian does not package -------------------------------------
# These two do the heavy lifting for "simpler", and neither is in the archive:
#
#   Just Perfection          removes shell elements outright. Everything else
#                            here restyles GNOME; this is the only thing that
#                            deletes parts of it.
#   Rounded Window Corners   GNOME square-corners every window regardless of
#                            GTK theme, and square corners against a rounded
#                            dock and rounded panel is the single loudest
#                            tell that this is a theme rather than a design.
#
# Installed from extensions.gnome.org by asking its API for the build that
# matches this shell — a zip for the wrong GNOME installs fine and then simply
# never loads, which presents as "the setting had no effect".
echo ">>> extensions not in Debian (just-perfection, rounded corners)"
SHELL_VER="$(run "gnome-shell --version | grep -oE '[0-9]+' | head -1" || echo 48)"
for uuid in just-perfection-desktop@just-perfection rounded-window-corners@fxgn; do
  url="$(run "curl -s --max-time 30 'https://extensions.gnome.org/extension-info/?uuid=$uuid&shell_version=$SHELL_VER' | python3 -c 'import sys,json; print(json.load(sys.stdin).get(\"download_url\",\"\"))'" || true)"
  if [[ -z "$url" || "$url" == "None" ]]; then
    echo "    WARNING: no build of $uuid for GNOME $SHELL_VER — skipping" >&2
    continue
  fi
  run "set -e
    tmp=\$(mktemp -d)
    curl -fsL --max-time 60 -o \$tmp/e.zip 'https://extensions.gnome.org$url'
    gnome-extensions install --force \$tmp/e.zip
    rm -rf \$tmp" \
    && echo "    installed $uuid" \
    || echo "    WARNING: $uuid failed to install" >&2
done

# Apple licenses SF Pro for use on Apple platforms; it is not ours to install
# here. Inter is the closest open face — same humanist grotesque skeleton, tall
# x-height, designed for UI at small sizes — and it is what the theme targets.
echo ">>> type: Inter"

# --- WhiteSur ---------------------------------------------------------------
# File-copy installs only: a GTK theme, a shell theme and an icon set, all into
# ~/.themes and ~/.icons. Deliberately NOT the variants that patch gnome-shell's
# gresource bundle — that edits a system file, survives uninstall badly, and a
# bad patch is a desktop that does not start. If the shell theme is wrong here,
# the fix is toggling one extension off.
echo ">>> WhiteSur (GTK + shell + icons)"
# TERM is not incidental. WhiteSur's installer drives the cursor with setterm
# and tput; over a non-interactive ssh there is no tty and no TERM, so it dies
# with "TERM environment variable not set" *after* printing its banner and
# *after* redirecting stderr to a log it then deletes on exit. The visible
# result is a clean-looking run that installs nothing, which reads as "the
# theme does not support this GNOME" rather than "there was no terminal".
THEME_OK=1
run 'set -e
  export TERM=xterm-256color
  rm -rf ~/.cache/whitesur && mkdir -p ~/.cache/whitesur ~/.themes ~/.icons
  cd ~/.cache/whitesur
  git clone --depth=1 -q https://github.com/vinceliuice/WhiteSur-gtk-theme.git
  git clone --depth=1 -q https://github.com/vinceliuice/WhiteSur-icon-theme.git
  git clone --depth=1 -q https://github.com/vinceliuice/WhiteSur-cursors.git
  cd WhiteSur-gtk-theme
  ./install.sh -c Light -c Dark -a normal -t default >/dev/null 2>&1
  # -l installs into ~/.config/gtk-4.0, which is the ONLY place libadwaita
  # reads from. Without it Files, Settings, Text Editor and every other modern
  # GNOME app ignore the theme entirely and render stock Adwaita — so the
  # desktop looks themed until you open a window, and then half of it does not.
  # The flag defaults to the dark sheet, hence the explicit -c Light.
  ./install.sh -c Light -a normal -t default -l >/dev/null 2>&1
  cd ../WhiteSur-icon-theme
  ./install.sh -a >/dev/null 2>&1
  cd ../WhiteSur-cursors
  ./install.sh >/dev/null 2>&1
' || THEME_OK=0

# Wallpaper carries more of the "which OS is this" signal than the widget theme
# does — it is the largest surface on screen and the only one visible before
# anything is open. GNOME's default blobs read as GNOME no matter what the
# window controls are doing.
echo ">>> wallpaper"
# The pack ships its own installers. Neither is used: install-gnome-backgrounds.sh
# exits 0 having written nothing reachable (it targets a system path and does
# not fail when it cannot), which is worse than an error — the caller sees
# success and then finds no wallpapers. A plain copy has no such ambiguity.
WALL=""
run 'set -e
  mkdir -p ~/.local/share/backgrounds/whitesur
  cd ~/.cache/whitesur
  rm -rf WhiteSur-wallpapers
  git clone --depth=1 -q https://github.com/vinceliuice/WhiteSur-wallpapers.git
  cd WhiteSur-wallpapers
  # 4k first; the guest display is resizable and a 1080p still upscales badly
  # on a Retina host.
  cp -f 4k/*.jpg ~/.local/share/backgrounds/whitesur/ 2>/dev/null \
    || cp -f 1080p/*.jpg ~/.local/share/backgrounds/whitesur/
  ls ~/.local/share/backgrounds/whitesur/ | wc -l
' >/dev/null 2>&1 || echo "    note: wallpaper pack unavailable; keeping the GNOME default"

# Monterey-light first, not Sonoma. Sonoma's still is a saturated orange-to-green
# sweep — it is the most *recent* macOS desktop, not the calmest, and it fights
# every icon placed on top of it. Monterey is a soft single-hue gradient, which
# is what lets the dock and the type be the things you notice.
WALL_DIR='~/.local/share/backgrounds/whitesur'
for pick in 'Monterey-light' 'Ventura-light' 'WhiteSur-light' 'Sonoma-light'; do
  WALL="$(run "ls $WALL_DIR/$pick.jpg 2>/dev/null | head -1" || true)"
  [[ -n "$WALL" ]] && break
done
[[ -n "$WALL" ]] || WALL="$(run "ls $WALL_DIR/*.jpg 2>/dev/null | head -1" || true)"
[[ -n "$WALL" ]] || WALL="/usr/share/backgrounds/gnome/adwaita-l.jpg"
WALL_DARK="$(run "ls $WALL_DIR/Sonoma-dark.jpg $WALL_DIR/Ventura-dark.jpg $WALL_DIR/*dark*.jpg 2>/dev/null | head -1" || true)"
[[ -n "$WALL_DARK" ]] || WALL_DARK="$WALL"
if [[ "$WALL" == /usr/share/backgrounds/gnome/* ]]; then
  echo "    WARNING: WhiteSur wallpapers missing; using the GNOME default" >&2
fi
echo "    light: $WALL"
echo "    dark:  $WALL_DARK"

if [[ "$THEME_OK" == 1 ]]; then
  # These three land in three different places. GTK themes go to ~/.themes,
  # but the icon and cursor installers prefer ~/.local/share/icons and only
  # fall back to ~/.icons — looking in one place finds the theme, misses the
  # icons, and reports a half-applied desktop as a successful one.
  GTK_THEME="$(run 'ls -d ~/.themes/WhiteSur-Light 2>/dev/null | head -1 | xargs -r basename' || true)"
  ICON_THEME="$(run 'ls -d ~/.local/share/icons/WhiteSur-light ~/.icons/WhiteSur-light 2>/dev/null | head -1 | xargs -r basename' || true)"
fi
# An empty theme name in dconf is not "use the default" — it is a broken
# lookup. Fall back to something that is definitely installed.
GTK_THEME="${GTK_THEME:-Adwaita}"
ICON_THEME="${ICON_THEME:-Papirus}"
if [[ "$GTK_THEME" == Adwaita ]]; then
  echo "    WARNING: WhiteSur did not install; this is the stock GNOME look," >&2
  echo "             not the one you asked for. Falling back to $GTK_THEME." >&2
fi
# Only point at the WhiteSur cursors if they actually landed — a dconf cursor
# name with nothing behind it silently resolves to the default, which makes a
# failed install look like a working one.
if run "test -d ~/.local/share/icons/WhiteSur-cursors -o -d ~/.icons/WhiteSur-cursors" 2>/dev/null; then
  CURSOR_THEME='WhiteSur-cursors'
else
  CURSOR_THEME='Adwaita'
fi
echo "    gtk:    $GTK_THEME"
echo "    icons:  $ICON_THEME"
echo "    cursor: $CURSOR_THEME"

# --- the actual look --------------------------------------------------------
echo ">>> writing desktop defaults"
# The keyfile itself comes from linux/iso/render-dconf.sh, which is the same
# generator the ISO build uses. Two copies of these eighty lines is a guarantee
# that the image and the machine you tested on disagree about something small.
DCONF="$(GTK_THEME="$GTK_THEME" ICON_THEME="$ICON_THEME" CURSOR_THEME="$CURSOR_THEME" \
         WALL="$WALL" WALL_DARK="$WALL_DARK" \
         FAVORITES="'chromium.desktop', 'teddyos-whatsapp.desktop', 'teddyos-gmail.desktop', 'org.gnome.Nautilus.desktop', 'org.gnome.Terminal.desktop'" \
         "$ROOT_DIR/linux/iso/render-dconf.sh")"

run "sudo mkdir -p /etc/dconf/db/local.d /etc/dconf/profile
sudo tee /etc/dconf/profile/user >/dev/null <<'EOF'
user-db:user
system-db:local
EOF
sudo tee /etc/dconf/db/local.d/00-teddyos >/dev/null <<'TEDDYOS_DCONF_EOF'
$DCONF
TEDDYOS_DCONF_EOF
sudo dconf update"

echo ">>> verifying the defaults compiled and resolve"
# dconf update is silent on a malformed keyfile — it just does not apply. Read
# a value back out of the compiled database rather than trusting the write.
run "test -f /etc/dconf/db/local || { echo '    ERROR: /etc/dconf/db/local was not compiled' >&2; exit 1; }
     got=\$(strings /etc/dconf/db/local | grep -c 'dash-to-dock' || true)
     [ \"\$got\" -gt 0 ] || { echo '    ERROR: dock settings missing from compiled db' >&2; exit 1; }
     echo '    dconf db ok'"

run "test -d ~/.themes/'$GTK_THEME' -o '$GTK_THEME' = Adwaita || {
       echo '    ERROR: gtk theme $GTK_THEME is not installed' >&2; exit 1; }
     echo '    theme present'"

cat <<DONE

>>> look applied.

    dock      bottom, centred, auto-hides, icons grow on hover
    menu bar  translucent + blurred
    buttons   close/min/max on the LEFT
    type      Inter
    theme     $GTK_THEME, $ICON_THEME icons, $CURSOR_THEME cursors

    Everything here is a *default*, so anything you dislike is one click in
    Settings or Tweaks away — nothing will overwrite your change.
DONE
