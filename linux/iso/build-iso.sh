#!/usr/bin/env bash
# Build a downloadable teddyOS live ISO — boot to try, install if you like.
#
# Runs INSIDE the Debian guest, not on the Mac. live-build needs debootstrap,
# a Debian archive keyring and a loop-capable kernel, none of which macOS has;
# the guest built by scripts/teddyos-vm.sh is already the right build host, and
# using it means the image is assembled by the same distro it targets.
#
# From the Mac, prefer the host wrapper so the commit is stamped correctly:
#
#   ./scripts/build-linux-iso.sh              # native arch of the guest
#   ./scripts/build-linux-iso.sh --arch amd64 # cross, via qemu-user binfmt
#   make linux-iso
#
# Direct use on the guest (when you already have the tree there):
#
#   TEDDYOS_COMMIT=abc1234 ./build-iso.sh
#   ./build-iso.sh --arch amd64
#
# Cross-building amd64 from arm64 works but every maintainer script in the
# chroot runs under qemu-user emulation, so expect it to take several times
# longer than the native build. That is the cost of one image that boots most
# PCs, and it is worth paying once per release rather than asking people to
# build their own.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"

ARCH="$(dpkg --print-architecture)"
DIST="${TEDDYOS_ISO_DIST:-trixie}"
MIRROR="${TEDDYOS_ISO_MIRROR:-http://deb.debian.org/debian/}"
OUT_DIR="${TEDDYOS_ISO_OUT:-$HOME/teddyos-iso}"
VERSION="$(cat "$REPO/VERSION" 2>/dev/null || echo 0.0.0)"
# Resolve the commit this image represents. Preference order:
#   1. TEDDYOS_COMMIT from the environment (host wrapper sets this — the guest
#      tree is usually scp'd without .git, so git below would fail)
#   2. $REPO/.teddyos-commit written by the host before the copy
#   3. git rev-parse when the tree actually is a clone
#   4. "unknown" — which teddyos-update treats as different from every
#      published commit, so a brand-new install would offer an update on boot
COMMIT="${TEDDYOS_COMMIT:-}"
if [[ -z "$COMMIT" && -f "$REPO/.teddyos-commit" ]]; then
  COMMIT="$(tr -d '[:space:]' <"$REPO/.teddyos-commit")"
fi
if [[ -z "$COMMIT" ]]; then
  COMMIT="$(cd "$REPO" && git rev-parse --short HEAD 2>/dev/null || true)"
fi
COMMIT="${COMMIT:-unknown}"
# Every build so far produced a file with an identical name, so there was no
# way — from the filename, the boot screen, or inside the running system — to
# tell which build you were looking at. Three fixes got reported as "still
# broken" because an older image was being booted. The stamp goes on the boot
# splash AND into /etc/teddyos-build so the question is answerable from either
# side.
BUILD_ID="$(date -u +%Y%m%d-%H%M)"
JOBS="$(nproc)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --out)  OUT_DIR="$2"; shift 2 ;;
    --help|-h)
      sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "error: unknown argument $1" >&2; exit 2 ;;
  esac
done

case "$ARCH" in
  amd64|arm64) ;;
  *) echo "error: --arch must be amd64 or arm64 (got $ARCH)" >&2; exit 2 ;;
esac

HOST_ARCH="$(dpkg --print-architecture)"
BUILD="$OUT_DIR/build-$ARCH"
IMAGE="$OUT_DIR/teddyos-${VERSION}-${ARCH}-${BUILD_ID}.iso"

echo ">>> teddyOS $VERSION  commit=$COMMIT  ->  $ARCH  ($DIST)"
[[ "$ARCH" != "$HOST_ARCH" ]] && echo "    cross-building from $HOST_ARCH; this is the slow path"
if [[ "$COMMIT" == "unknown" ]]; then
  echo "    warning: commit is unknown — pass TEDDYOS_COMMIT or run via" >&2
  echo "             scripts/build-linux-iso.sh so the image does not offer" >&2
  echo "             a false update on first boot" >&2
fi

# --- build host ---------------------------------------------------------
echo ">>> build dependencies"
sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq
PKGS="live-build debootstrap xorriso squashfs-tools ca-certificates rsync librsvg2-bin"
# Foreign-architecture chroots run their maintainer scripts through qemu-user,
# registered with the kernel by binfmt-support. Without both, debootstrap's
# second stage dies on the first postinst with "Exec format error".
[[ "$ARCH" != "$HOST_ARCH" ]] && PKGS="$PKGS qemu-user-static binfmt-support"
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PKGS

if [[ "$ARCH" != "$HOST_ARCH" ]]; then
  sudo systemctl restart systemd-binfmt 2>/dev/null || true
  target_qemu="qemu-x86_64"; [[ "$ARCH" == arm64 ]] && target_qemu="qemu-aarch64"
  if [[ ! -e "/proc/sys/fs/binfmt_misc/$target_qemu" ]]; then
    echo "error: binfmt handler $target_qemu is not registered — the foreign" >&2
    echo "       chroot cannot run its maintainer scripts. Check that" >&2
    echo "       binfmt-support and qemu-user-static are installed." >&2
    exit 1
  fi
  echo "    binfmt: $target_qemu registered"
fi

# Unmount before removing. An interrupted build — Ctrl-C, a killed process, a
# VM that stopped — leaves /proc, /sys and /dev bind-mounted INSIDE the chroot.
# `rm -rf` across a live mount fails with "Operation not permitted", and under
# `set -e` that kills the next build during cleanup, before it has done
# anything at all. The log then shows a wall of rm errors about /sys/module/...
# which looks like a permissions problem and is really a leftover mount.
#
# Reverse-sorted so nested mounts come off before their parents.
if [[ -d "$BUILD" ]]; then
  awk -v b="$BUILD/" 'index($2, b) == 1 {print $2}' /proc/mounts \
    | sort -r | while read -r mp; do sudo umount -lf "$mp" 2>/dev/null || true; done
fi

# sudo, because live-build creates the chroot as root. Without it this rm fails
# under `set -e` and the run dies before doing anything — while the PREVIOUS
# build tree stays on disk, so the stale log still says "hook failed" and you
# spend an hour debugging a hook that never re-ran.
sudo rm -rf "$BUILD"
mkdir -p "$BUILD" "$OUT_DIR"
cd "$BUILD"

# --- configure ----------------------------------------------------------
echo ">>> lb config"
lb config \
  --architectures "$ARCH" \
  --distribution "$DIST" \
  --archive-areas "main contrib non-free non-free-firmware" \
  --mirror-bootstrap "$MIRROR" \
  --mirror-binary "$MIRROR" \
  --debian-installer live \
  --debian-installer-gui true \
  --iso-application "teddyOS" \
  --iso-volume "teddyOS $VERSION" \
  --iso-publisher "teddyOS; https://teddysearch.com" \
  --memtest none \
  --apt-recommends false \
  --backports false \
  --binary-images iso-hybrid \
  --bootappend-live "boot=live components quiet splash username=teddy hostname=teddyos console=ttyS0,115200n8 console=ttyAMA0,115200n8 console=tty0" \
  >/dev/null

# non-free-firmware is deliberate. Debian split firmware out in trixie, and an
# image without it boots to a black screen or a dead wifi card on a large slice
# of real laptops — which is indistinguishable, to someone trying this for the
# first time, from the OS being broken.
#
# A note on a failure that will look like a bug and is not one.
#
# The amd64 debian-installer stage has twice died with apt reporting
#
#   rename failed, No such file or directory
#     (/binary.deb/archives/partial/X -> /binary.deb/archives/X)
#
# which reads as live-build pointing apt at a directory it forgot to create.
# It is not. That message appears when the DOWNLOAD failed, so there is no
# partial/X left to move — and both times the log carried an order of magnitude
# more "Temporary failure resolving deb.debian.org" than rename errors. The
# build host's network dropped; apt named whichever parallel fetch finished
# first. Re-run before changing anything here.
#
# Cross-building is what makes this worth writing down: the run takes long
# enough that a momentary DNS outage on the build host is likely rather than
# rare, and its symptom points at the wrong layer.
#
# tty0 is listed LAST, and the order is the whole point: every console named
# gets the kernel log, but the LAST one becomes /dev/console — which is where
# systemd's status and any emergency shell go.
#
# This was originally written the other way round, with tty0 first. That put
# /dev/console on a serial port on every machine that has one — every QEMU and
# UTM guest has a PL011 — so a boot that failed would drop its emergency shell
# somewhere nobody is looking and show the user a black screen. Naming the
# screen last keeps the log on serial AND keeps the failure visible.
#
# Both ttyS0 and ttyAMA0 are named because the serial port is not the same
# device on the two architectures this image ships for. x86 has the 16550 at
# ttyS0; aarch64 virtual machines expose a PL011 at ttyAMA0. Naming only one
# covers half the builds, and the half it misses is the half that boots to a
# black screen. A console that does not exist on a given machine is ignored by
# the kernel, so naming both costs nothing.
#
# The serial console exists because of an afternoon spent guessing. A VM that
# boots to a black rectangle gives you nothing — no panic, no progress, no way
# to tell "hung" from "running fine and simply not drawing". Every hypervisor
# can point a serial port at a file, so this turns that silence into a log for
# free, and costs a machine with no serial port nothing at all.

mkdir -p config/package-lists config/includes.chroot config/hooks/live

# --- boot menu ----------------------------------------------------------
# Two defects in the stock live-build boot screen, both of which a stranger
# meets before anything else:
#
#   1. There is no timeout. `config.cfg` sets `default=0` and never sets
#      `timeout`, so GRUB waits forever. Someone who boots the image and walks
#      away comes back to the identical screen, and it reads as a hang — which
#      is exactly how it was reported.
#   2. It is branded Debian, with the Debian swirl, for an OS called teddyOS.
#
# live-build reads these from config/bootloaders/, falling back to its own
# templates. Copy the templates and patch them rather than writing from
# scratch: they carry the gfxmode/efi_gop probing that makes the menu appear at
# all on odd firmware, and reproducing that badly means a black screen.
echo ">>> boot menu"
mkdir -p config/bootloaders
cp -r /usr/share/live/build/bootloaders/grub-pc config/bootloaders/
BOOTCFG=config/bootloaders/grub-pc/config.cfg
grep -q '^set timeout' "$BOOTCFG" || sed -i '1a set timeout=5\nset timeout_style=menu' "$BOOTCFG"
sed -i 's|title-text: .*|title-text: "teddyOS"|' \
  config/bootloaders/grub-pc/live-theme/theme.txt

# The splash. Rendered from SVG here rather than committed as a PNG so the
# version string is always the one being built — a stale number silkscreened
# into an image is the kind of wrong that survives for releases.
cat > /tmp/teddyos-splash.svg <<SPLASH
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#12131a"/>
      <stop offset="100%" stop-color="#07070a"/>
    </linearGradient>
  </defs>
  <rect width="800" height="600" fill="url(#bg)"/>
  <!-- Kept to the top strip: GRUB draws the menu over the middle of this
       image, and anything behind the entries makes them unreadable. -->
  <text x="60" y="86" font-family="DejaVu Sans, sans-serif" font-size="46"
        font-weight="600" fill="#ffffff">teddyOS</text>
  <text x="60" y="118" font-family="DejaVu Sans, sans-serif" font-size="17"
        fill="#8b8f9e">$VERSION · $ARCH · build $BUILD_ID</text>
  <rect x="60" y="140" width="680" height="1" fill="#23252e"/>
</svg>
SPLASH
if command -v rsvg-convert >/dev/null; then
  rsvg-convert -w 800 -h 600 -o config/bootloaders/grub-pc/splash.png /tmp/teddyos-splash.svg
  echo "    splash rendered"
else
  echo "    WARNING: rsvg-convert missing; keeping the Debian splash" >&2
fi

# --- and the SAME two fixes again, for BIOS ---------------------------------
# Everything above patches GRUB. On amd64 that is only half the machine.
#
# `lb config` is not given --bootloaders, so it uses its default: arm64 gets
# grub-efi alone, but amd64 gets `syslinux,grub-efi` — two independent boot
# menus in one ISO. Which one a visitor sees is decided by their firmware, not
# by us. UEFI reads boot/grub/; legacy BIOS and CSM read isolinux/ — and
# VirtualBox on x86 defaults to BIOS, which is very likely the single most
# common way this image will ever be opened.
#
# Left alone, that path keeps live-build's stock templates: the Debian swirl,
# entries reading "Live system (amd64)", and `timeout 0`, which in syslinux
# means wait forever rather than proceed immediately. That is precisely the two
# defects described at the top of this section, shipped intact to the audience
# most likely to hit them.
if [[ -d /usr/share/live/build/bootloaders/isolinux ]]; then
  cp -r /usr/share/live/build/bootloaders/isolinux config/bootloaders/
  # The template ships exactly three files — isolinux.bin, isolinux.cfg and
  # ldlinux.c32 — and isolinux.cfg is a four-line stub that `include`s a
  # menu.cfg live-build generates later, at build time, into the binary tree.
  # So only the timeout is fixable here; the entry titles do not exist yet and
  # are renamed by the 0600 binary hook once they do.
  #
  # syslinux counts in tenths of a second, so 50 is the 5s the GRUB menu uses.
  # Written unconditionally rather than only when it reads `timeout 0`: the
  # stock value can change, and any menu without a timeout is the defect.
  sed -i 's/^timeout .*/timeout 50/' config/bootloaders/isolinux/isolinux.cfg
  if command -v rsvg-convert >/dev/null; then
    # 640x480 because vesamenu's background, unlike GRUB's, is a fixed mode.
    rsvg-convert -w 640 -h 480 -o config/bootloaders/isolinux/splash.png /tmp/teddyos-splash.svg
    echo "    splash rendered (BIOS)"
  fi
fi
rm -f /tmp/teddyos-splash.svg

# --- what goes in it ----------------------------------------------------
cat > config/package-lists/teddyos.list.chroot <<'PKGS'
# Desktop. gnome-core, not the gnome task: the task pulls games, and the first
# impression of a new OS should not be that it came with solitaire.
gnome-core
gdm3
gnome-tweaks
gnome-shell-extension-dashtodock
gnome-shell-extension-blur-my-shell
gnome-shell-extension-light-style
gnome-shell-extension-appindicator
gnome-shell-extensions

# The X11 fallback, and it is not optional.
#
# GNOME's normal session is Wayland, which requires a DRM/KMS device. Some
# machines do not have one — VirtualBox's Apple Silicon preview exposes its
# framebuffer only through EFI GOP, so the kernel registers `efi-framebuffer.0`,
# simpledrm has nothing to bind to, /dev/dri never appears. On those machines
# GDM disables Wayland, looks for an X session, finds none, and dies with
#   Gdm: GdmSession: no session desktop files installed, aborting.
# restarting forever behind a black screen. The console works the whole time,
# which is what makes it so confusing to diagnose.
#
# gnome-session-xsession is what puts gnome-xorg.desktop in /usr/share/xsessions
# and it is only a Recommends of gnome-core — so --apt-recommends false below
# silently removes it. Same failure mode as user-setup: nothing errors at build
# time, the ISO boots, and the desktop simply never appears.
#
# xserver-xorg-video-fbdev is the driver that makes this actually work: it draws
# on /dev/fb0 and needs no KMS. GNOME Shell keeps its X11 backend in 48
# (`gnome-shell --x11`), so this is the real desktop — dock, theme and
# extensions included — not a reduced one.
gnome-session-xsession
xserver-xorg-core
xserver-xorg-video-fbdev
xserver-xorg-input-libinput

# Look
fonts-inter
fonts-noto
fonts-noto-color-emoji
papirus-icon-theme

# The day-one bar
chromium
git
# GitHub CLI: Search uses it for "work on tsearch" when there is no local
# folder — clone the matching repo if the user is signed in. Without gh,
# SSH keys + github.user still work for owner/name probes.
gh
ripgrep
curl
ca-certificates
python3
python3-gi
gir1.2-gtk-4.0
# The terminal widget behind the Claude window. Claude Code is a terminal
# program; this is what lets it live in an app instead of a shell.
gir1.2-vte-3.91
gir1.2-adw-1

# sudo belongs in any desktop image — the installer puts the new account in
# the sudo group and it would otherwise be unable to administer its own
# machine. It is ALSO a build dependency here: WhiteSur's installer does
# `SUDO_BIN=$(which sudo)` under `set -Eeo pipefail`, so its absence fails the
# whole script with exit 1 and no message.
sudo

# Build-time tools the chroot hooks need. Not runtime dependencies — but the
# hooks run INSIDE the chroot, so "it works on the dev VM" proves nothing about
# whether they exist here. sassc builds the WhiteSur stylesheets; unzip unpacks
# the GNOME extensions; glib-compile-schemas registers their settings.
sassc
unzip
# Node ships .tar.xz and the minimal chroot has tar but not the xz backend, so
# extraction dies with "xz: Cannot exec" — which reads as a broken tarball.
xz-utils
libglib2.0-bin
libglib2.0-dev-bin

# THE reason the live session works at all. live-config's 0030-user-setup
# shells out to `user-setup` to create the live user at boot; live-config only
# Recommends it, and this build passes --apt-recommends false, so it was
# silently absent. The symptom is not an error anywhere — the live ISO boots
# perfectly and lands on a GDM screen with an empty Username field, because
# there are no users. Nobody downloading an image knows a username, so the
# whole thing is a dead end that looks like a working boot.
user-setup

# Audio, and the installer the live session offers
pipewire-audio
wireplumber
calamares
calamares-settings-debian
PKGS

# The search engine, its capability model, and the first-boot screen.
install -Dm644 "$REPO/linux/teddyos-search/caps.py"       config/includes.chroot/usr/lib/teddyos/caps.py
install -Dm644 "$REPO/linux/teddyos-search/search.py"     config/includes.chroot/usr/lib/teddyos/search.py
install -Dm644 "$REPO/linux/teddyos-search/sandbox.py"    config/includes.chroot/usr/lib/teddyos/sandbox.py
install -Dm644 "$REPO/linux/teddyos-search/work_tools.py"    config/includes.chroot/usr/lib/teddyos/work_tools.py
install -Dm644 "$REPO/linux/teddyos-search/git_projects.py"  config/includes.chroot/usr/lib/teddyos/git_projects.py
install -Dm755 "$REPO/linux/teddyos-search/teddyos-search" config/includes.chroot/usr/bin/teddyos-search
install -Dm755 "$REPO/linux/teddyos-setup/teddyos-setup"   config/includes.chroot/usr/bin/teddyos-setup
install -Dm755 "$REPO/linux/teddyos-search/teddyos-search-app" config/includes.chroot/usr/bin/teddyos-search-app
install -Dm755 "$REPO/linux/teddyos-setup/teddyos-welcome"       config/includes.chroot/usr/bin/teddyos-welcome
install -Dm755 "$REPO/linux/teddyos-claude/teddyos-claude"      config/includes.chroot/usr/bin/teddyos-claude
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-agent"        config/includes.chroot/usr/bin/teddyos-agent
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-ask-all"      config/includes.chroot/usr/bin/teddyos-ask-all
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-perplexity"   config/includes.chroot/usr/bin/teddyos-perplexity
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-devin"       config/includes.chroot/usr/bin/teddyos-devin
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-replit"      config/includes.chroot/usr/bin/teddyos-replit
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-accounts"     config/includes.chroot/usr/bin/teddyos-accounts
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-open-signin"  config/includes.chroot/usr/bin/teddyos-open-signin
# LinkedIn / WhatsApp messaging wrappers — Search opens these when someone
# asks to reply to messages (draft-only; never auto-send).
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-linkedin"     config/includes.chroot/usr/bin/teddyos-linkedin
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-whatsapp"     config/includes.chroot/usr/bin/teddyos-whatsapp
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-gmail"        config/includes.chroot/usr/bin/teddyos-gmail
# Highest available display mode at login (VM-friendly; no Settings homework).
install -Dm755 "$REPO/linux/teddyos-agent/teddyos-display"      config/includes.chroot/usr/bin/teddyos-display
install -Dm755 "$REPO/linux/teddyos-update/teddyos-update"      config/includes.chroot/usr/bin/teddyos-update
# Account / sign-in helpers used by teddyos-accounts and Search.
install -Dm644 "$REPO/linux/teddyos-search/accounts.py" \
  config/includes.chroot/usr/lib/teddyos/accounts.py
install -Dm644 "$REPO/linux/teddyos-search/audience.py" \
  config/includes.chroot/usr/lib/teddyos/audience.py
install -Dm644 "$REPO/linux/teddyos-search/progress.py" \
  config/includes.chroot/usr/lib/teddyos/progress.py
install -Dm644 "$REPO/linux/teddyos-search/pending_ask.py" \
  config/includes.chroot/usr/lib/teddyos/pending_ask.py
install -Dm644 "$REPO/linux/teddyos-search/intent.py" \
  config/includes.chroot/usr/lib/teddyos/intent.py

# --- logs -------------------------------------------------------------------
# Persistent journal + per-app files + daily snapshots. Without this, a failed
# first boot evaporates on reboot and Search freezes leave no trail.
install -Dm644 "$REPO/linux/logging/logutil.py" \
  config/includes.chroot/usr/lib/teddyos/logutil.py
install -Dm755 "$REPO/linux/logging/teddyos-log-collect" \
  config/includes.chroot/usr/bin/teddyos-log-collect
install -Dm644 "$REPO/linux/logging/journald-teddyos.conf" \
  config/includes.chroot/etc/systemd/journald.conf.d/teddyos.conf
install -Dm644 "$REPO/linux/logging/logrotate-teddyos" \
  config/includes.chroot/etc/logrotate.d/teddyos
install -Dm644 "$REPO/linux/logging/tmpfiles-teddyos.conf" \
  config/includes.chroot/usr/lib/tmpfiles.d/teddyos.conf
install -Dm644 "$REPO/linux/logging/teddyos-log-collect.service" \
  config/includes.chroot/etc/systemd/system/teddyos-log-collect.service
install -Dm644 "$REPO/linux/logging/teddyos-log-collect.timer" \
  config/includes.chroot/etc/systemd/system/teddyos-log-collect.timer
install -Dm644 "$REPO/linux/logging/README" \
  config/includes.chroot/var/log/teddyos/README
# Sticky app dir so user sessions can write without root.
mkdir -p config/includes.chroot/var/log/teddyos/app \
         config/includes.chroot/var/log/teddyos/snapshots
chmod 1777 config/includes.chroot/var/log/teddyos/app

# The shell extension: hides quick-settings toggles teddyOS has no reason to
# offer, and renames "Wired" to "Internet" in the panel and its menu.
mkdir -p config/includes.chroot/usr/share/gnome-shell/extensions
cp -r "$REPO/linux/shell-extension/teddyos@teddysearch.com" \
  config/includes.chroot/usr/share/gnome-shell/extensions/

# Get Started: shown once after setup, findable by name afterwards.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-welcome.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Get Started
Comment=A short tour of teddyOS
Exec=teddyos-welcome
Icon=teddyos-search
Terminal=false
Categories=System;
StartupWMClass=com.teddyos.Welcome
DESKTOP

install -Dm644 /dev/stdin config/includes.chroot/etc/xdg/autostart/teddyos-welcome.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=teddyOS Get Started
Exec=teddyos-welcome --if-needed
OnlyShowIn=GNOME;
X-GNOME-Autostart-Phase=Applications
NoDisplay=true
DESKTOP

# Max display resolution once the session (and Mutter) is up. Delay so gdctl
# can talk to DisplayConfig; persistent so the choice sticks across logins.
install -Dm644 /dev/stdin config/includes.chroot/etc/xdg/autostart/teddyos-display.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=teddyOS Display
Comment=Use the highest available screen resolution
Exec=teddyos-display
OnlyShowIn=GNOME;
X-GNOME-Autostart-Phase=Application
X-GNOME-Autostart-Delay=2
NoDisplay=true
DESKTOP
# Both entry points import from /usr/lib/teddyos once installed, not from a
# sibling directory in a git checkout.
sed -i 's|sys.path.insert(0, str(Path(__file__).resolve().parent))|sys.path.insert(0, "/usr/lib/teddyos")|' \
  config/includes.chroot/usr/bin/teddyos-search
sed -i '\|teddyos-search"))|d' config/includes.chroot/usr/bin/teddyos-setup
sed -i '\|resolve().parent))|d' config/includes.chroot/usr/bin/teddyos-search-app
sed -i '\|resolve().parent.parent|d' config/includes.chroot/usr/bin/teddyos-welcome

# The Search window. Without a .desktop it is not in the dock, not in the app
# grid, and not findable — i.e. it may as well not exist for anyone who does
# not already know to type its name.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-search.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Search
Comment=Ask teddyOS anything
Exec=teddyos-search-app
Icon=teddyos-search
Terminal=false
Categories=Utility;
StartupWMClass=com.teddyos.Search
DESKTOP

# The browser, relabelled. Chromium ships as "Chromium" with a desaturated
# Chrome logo — people recognise Chrome's four-colour pinwheel, but the grey-
# blue one reads as an unfamiliar app, and this is the icon someone reaches for
# most. "Web" with a globe says what it does to anyone who has used a computer.
# The real Chromium launcher stays installed for anyone looking for it by name.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-web.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Web
Comment=Browse the internet
Exec=chromium %U
Icon=teddyos-web
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=chromium
DESKTOP

# WhatsApp. The dock has listed teddyos-whatsapp.desktop as a favourite since
# the beginning and this file was never created, so GNOME silently dropped it
# and the dock has been showing three icons where four were intended. A
# favourite pointing at a launcher that does not exist fails quietly — nothing
# logs it, the icon simply is not there.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-whatsapp.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=WhatsApp
Comment=Messages
Exec=teddyos-whatsapp
Icon=teddyos-whatsapp
Terminal=false
Categories=Network;InstantMessaging;
StartupWMClass=teddyos-whatsapp
DESKTOP

# Gmail opens a setup guide first — not a naked mail.google.com tab.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-gmail.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Gmail
Comment=Set up Gmail on teddyOS
Exec=teddyos-gmail
Icon=gmail-desktop
Terminal=false
Categories=Network;Email;
StartupNotify=true
DESKTOP

# Claude desktop entry — installed but hidden from the dock/app grid.
#
# Non-technical people must not land in a black terminal-shaped window from the
# dock. The path is Search → work on … → Get help (Answers). Engineers can still
# run teddyos-claude from a shell if they want the full interactive UI.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-claude.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Claude
Comment=Ask Claude (via Search)
Exec=teddyos-search-app
Icon=teddyos-claude
Terminal=false
NoDisplay=true
Categories=Utility;Development;
StartupWMClass=com.teddyos.Claude
DESKTOP

# Perplexity as a one-tap web app — non-technical people should not have to
# find a URL or install an AppImage by hand.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-perplexity.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Perplexity
Comment=Ask anything on the web
Exec=teddyos-perplexity
Icon=teddyos-perplexity
Terminal=false
Categories=Network;WebBrowser;
StartupWMClass=teddyos-perplexity
DESKTOP

# Devin (Cognition) — web app wrapper.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-devin.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Devin
Comment=AI software engineer from Cognition
Exec=teddyos-devin
Icon=teddyos-devin
Terminal=false
Categories=Network;Development;
StartupWMClass=teddyos-devin
DESKTOP

# Replit — web app / Agent.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-replit.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Replit
Comment=Build apps from plain words
Exec=teddyos-replit
Icon=teddyos-replit
Terminal=false
Categories=Network;Development;
StartupWMClass=teddyos-replit
DESKTOP

# Getting you ready — sign-in walkthrough (opened from Search when needed).
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-accounts.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Getting you ready
Comment=One-time sign-in so teddyOS can help
Exec=teddyos-accounts
Icon=teddyos-accounts
Terminal=false
NoDisplay=true
Categories=Settings;Utility;
StartupWMClass=com.teddyos.Accounts
DESKTOP

# Answers window (Get help / ask every ready AI). Without this, the shell
# shows the raw app id "com.teddyos.AskAll" and a generic gear icon.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-answers.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Answers
Comment=Answers to what you asked, in one place
Exec=teddyos-ask-all
Icon=teddyos-answers
Terminal=false
NoDisplay=true
Categories=Utility;
StartupWMClass=com.teddyos.AskAll
DESKTOP

# Install teddyOS.
#
# The live image had no way to keep it. The boot menu offers an installer, but
# once someone is looking at the desktop and has decided they like it, there is
# nothing on screen that says "you can keep this" — they would have to reboot
# and know to pick a different menu entry. That is the whole conversion step,
# missing.
#
# Its own launcher rather than the one calamares-settings-debian ships, because
# that one is called "Install Debian" and shows a Debian icon. Someone who has
# spent ten minutes in teddyOS being told this is teddyOS should not be asked,
# at the moment of committing their disk, to install something they have never
# heard of.
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-install.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Install teddyOS
Comment=Keep teddyOS on this computer
Exec=calamares-install-debian
Icon=teddyos-install
Terminal=false
Categories=System;
StartupWMClass=calamares
DESKTOP

# Take the installer back out of the dock once it has done its job.
#
# The dock default ships with Install teddyOS pinned, because on the live image
# that is the one action with no other route to it. On an installed machine it
# is an icon offering to install the thing you are already running.
#
# Detected by /run/live/medium, which live-boot creates and an installed system
# does not have. Fails in the harmless direction: if this never runs, someone
# has a spare icon they can unpin, rather than a live user with no way to keep
# the OS.
install -Dm755 /dev/stdin config/includes.chroot/usr/lib/teddyos/dock-install-icon <<'SCRIPT'
#!/bin/sh
[ -d /run/live/medium ] && exit 0     # still live: leave it pinned
current=$(gsettings get org.gnome.shell favorite-apps 2>/dev/null) || exit 0
case "$current" in
  *teddyos-install.desktop*) ;;
  *) exit 0 ;;                        # already gone
esac
printf '%s' "$current" \
  | sed "s/, *'teddyos-install.desktop'//; s/'teddyos-install.desktop', *//" \
  | xargs -0 gsettings set org.gnome.shell favorite-apps
SCRIPT

install -Dm644 /dev/stdin config/includes.chroot/etc/xdg/autostart/teddyos-dock-install.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=teddyOS dock tidy
Exec=/usr/lib/teddyos/dock-install-icon
OnlyShowIn=GNOME;
X-GNOME-Autostart-Phase=Applications
NoDisplay=true
DESKTOP

# Chromium's search box points at teddysearch, not DuckDuckGo. A managed
# policy applies to every profile including each dock web app's own
# --user-data-dir, and survives a profile reset.
install -Dm644 "$HERE/chromium-policy.json" \
  config/includes.chroot/etc/chromium/policies/managed/teddyos.json

# The bookmark bar. Debian's master_preferences imports its bookmarks from
# /usr/share/chromium/initial_bookmarks.html, which ships Debian.org, Latest
# News and Help — three links about the distribution underneath, on a bar the
# person sees every time they open a window.
#
# Replacing that file rather than using the ManagedBookmarks policy is
# deliberate, and for the same reason the search provider is a default rather
# than a lock: ManagedBookmarks renders an uneditable folder, and a bar someone
# cannot rearrange is a worse first impression than a bar with the wrong links.
# Seeded this way they are ordinary bookmarks — draggable, deletable, theirs.
install -Dm644 "$HERE/initial_bookmarks.html" \
  config/includes.chroot/usr/share/chromium/initial_bookmarks.html

# --- the installer, wearing the right name ----------------------------------
#
# Clicking Install teddyOS opened a window titled "Debian GNU/Linux Installer",
# with a Debian swirl and "Welcome to the Calamares installer for Debian 13".
# Every other branding leak costs a moment of confusion; this one arrives at the
# moment somebody is about to hand over their disk, and tells them they are
# installing something they have never heard of.
# The login banner. getty prints /etc/issue above the prompt, and Debian's
# reads "Debian GNU/Linux 13 \\n \\l" — so every text console, every serial
# session and every failed-boot rescue shell announces a distribution the
# person has never chosen. Found on the amd64 serial console, which is the one
# place nobody thought to look.
#
# \\n and \\l are getty escapes for the hostname and the tty name; they are
# kept because "which machine, which terminal" is the one useful thing this
# line says.
install -Dm644 /dev/stdin config/includes.chroot/etc/issue <<'ISSUE'
teddyOS \n \l

ISSUE
install -Dm644 /dev/stdin config/includes.chroot/etc/issue.net <<'ISSUENET'
teddyOS
ISSUENET

install -Dm644 "$HERE/calamares-branding.desc" \
  config/includes.chroot/etc/calamares/branding/teddyos/branding.desc
# Required, not decorative: a branding component that declares slideshowAPI
# without a slideshow is rejected wholesale, and Calamares then exits before
# drawing anything — which presents as the Install button doing nothing at all.
install -Dm644 "$HERE/calamares-show.qml" \
  config/includes.chroot/etc/calamares/branding/teddyos/show.qml
if command -v rsvg-convert >/dev/null; then
  rsvg-convert -w 512 -h 512 -o /tmp/teddyos-logo.png "$REPO/linux/icons/teddyos-logo.svg"
  install -Dm644 /tmp/teddyos-logo.png \
    config/includes.chroot/etc/calamares/branding/teddyos/teddyos-logo.png
  rm -f /tmp/teddyos-logo.png
else
  echo "    WARNING: rsvg-convert missing; installer keeps the Debian logo" >&2
fi

# App icons this OS draws for itself.
#
# They were `Icon=system-search` and `Icon=web-browser`, which the icon theme
# resolves to a thin symbolic magnifier and — the reason this matters — to
# SAFARI's compass. The dock was showing Apple's browser mark on Chromium.
#
# Installed into hicolor (not WhiteSur) so they survive theme changes.
# Scalable SVG + raster PNGs: dash-to-dock and some themes resolve PNGs more
# reliably than SVG-only names, so we bake 48/64/128/256 when rsvg-convert is
# available on the build host.
# One visual language for Search / Web / Messages / AIs / Answers / Install.
for icon in \
  teddyos-search teddyos-web teddyos-whatsapp teddyos-accounts \
  teddyos-claude teddyos-grok teddyos-gemini teddyos-codex \
  teddyos-copilot teddyos-antigravity teddyos-perplexity teddyos-cursor \
  teddyos-devin teddyos-replit \
  teddyos-answers teddyos-install teddyos-github
do
  src="$REPO/linux/icons/$icon.svg"
  if [[ ! -f "$src" ]]; then
    echo "ERROR: missing icon $src" >&2
    exit 1
  fi
  install -Dm644 "$src" \
    "config/includes.chroot/usr/share/icons/hicolor/scalable/apps/$icon.svg"
  if command -v rsvg-convert >/dev/null; then
    for size in 48 64 128 256; do
      rsvg-convert -w "$size" -h "$size" -o "/tmp/${icon}-${size}.png" "$src"
      install -Dm644 "/tmp/${icon}-${size}.png" \
        "config/includes.chroot/usr/share/icons/hicolor/${size}x${size}/apps/${icon}.png"
      rm -f "/tmp/${icon}-${size}.png"
    done
  fi
done
# Empty index.theme marker is not required for hicolor; cache is rebuilt in a
# chroot hook so the live image does not ship a stale/missing icon cache.

# Which build this is, readable from inside the running system. `cat
# /etc/teddyos-build` settles "am I testing the thing you just fixed?" without
# needing to remember what the boot splash said.
# What software this image shipped with, in the format teddyos-update reads.
# $COMMIT is resolved once at the top of this script (env / stamp file / git).
install -Dm644 /dev/stdin config/includes.chroot/etc/teddyos-software <<SOFTWARE
version=$VERSION
commit=$COMMIT
applied=$(date -u +%Y-%m-%dT%H:%M:%SZ)
SOFTWARE

install -Dm644 /dev/stdin config/includes.chroot/etc/teddyos-build <<BUILDINFO
TEDDYOS_VERSION=$VERSION
TEDDYOS_BUILD=$BUILD_ID
TEDDYOS_COMMIT=$COMMIT
TEDDYOS_ARCH=$ARCH
BUILDINFO

# The built-in corpus, so `search.query` has something to answer from offline.
if [[ -f "$REPO/search/seed.json" ]]; then
  install -Dm644 "$REPO/search/seed.json" config/includes.chroot/usr/share/teddyos/corpus.json
fi

# First boot runs the capability screen exactly once. --if-needed makes that
# true without a separate "have we run yet" file to get out of sync.
install -Dm644 /dev/stdin config/includes.chroot/etc/xdg/autostart/teddyos-setup.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=teddyOS Setup
Exec=teddyos-setup --if-needed
OnlyShowIn=GNOME;
X-GNOME-Autostart-Phase=Applications
NoDisplay=true
DESKTOP

# --- software updates -------------------------------------------------------
#
# Everything teddyOS adds sits in the squashfs, so before this a fixed bug
# reached an existing machine only by downloading 2.6 GB and installing the
# computer again. What changes between releases is a few hundred kilobytes;
# teddyos-update moves that much instead.
#
# Auto-install is OFF by default. The system timer always fires, but
# `teddyos-update auto` no-ops unless setup granted “Keep teddyOS up to date”
# (software.auto_update) — same consent shape as diagnostics.share.
install -Dm644 /dev/stdin config/includes.chroot/etc/systemd/system/teddyos-update.service <<'UNIT'
[Unit]
Description=Apply teddyOS software updates when the user allowed it
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
ExecStart=/usr/bin/teddyos-update auto
Nice=10
IOSchedulingClass=best-effort
IOSchedulingPriority=7
UNIT

install -Dm644 /dev/stdin config/includes.chroot/etc/systemd/system/teddyos-update.timer <<'UNIT'
[Unit]
Description=Daily teddyOS software update (only installs if allowed in setup)
[Timer]
# After the network is usually up; Persistent catches machines that sleep a lot.
OnBootSec=15min
OnUnitActiveSec=1d
Persistent=true
Unit=teddyos-update.service
[Install]
WantedBy=timers.target
UNIT

# Manual path for anyone who left auto-update off (or wants it now).
install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-update.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Software Update
Comment=Check for and install teddyOS updates
Exec=pkexec /usr/bin/teddyos-update apply
Icon=software-update-available
Terminal=false
Categories=System;
DESKTOP

# --- when there is no graphics device, say so ------------------------------
# GNOME needs a DRM/KMS device. Some machines do not have one, and when that
# happens GDM starts, finds nothing it can draw on, aborts with "no session
# desktop files installed", and restarts — forever. The screen stays black.
#
# Found on VirtualBox's Apple Silicon preview, which exposes its framebuffer
# only through EFI GOP: the kernel registers `efi-framebuffer.0` rather than a
# `simple-framebuffer`, so simpledrm cannot bind, /dev/dri never appears, and
# the console works perfectly while the desktop cannot start at all. Nothing in
# userspace or on the kernel command line changes that.
#
# A black screen tells the person nothing and reads as "this OS is broken". The
# machine knows exactly what is wrong, so it should say it, in the same plain
# language as everything else here.
install -Dm755 /dev/stdin config/includes.chroot/usr/lib/teddyos/no-gpu-notice <<'NOTICE'
#!/bin/sh
# Printed on the console, because there is by definition no desktop to show it
# in. Kept inside 80 columns so it does not wrap on a default text console.
exec >/dev/tty1 2>&1
printf '\033c'
cat <<'MSG'

  teddyOS could not start its desktop.

  This computer does not offer a graphics device teddyOS can draw on. That
  is a limit of the machine it is running on, not a fault in your files —
  nothing has been changed and nothing is damaged.

  If this is VirtualBox on an Apple Silicon Mac: its ARM support is still a
  preview and does not provide one yet. UTM does, and runs teddyOS as
  intended — https://mac.getutm.app

  On a real computer, this usually means the graphics driver is missing.

  A text console is on Alt+F2 if you want to look around.

MSG
NOTICE

install -Dm644 /dev/stdin config/includes.chroot/usr/lib/systemd/system/teddyos-no-gpu.service <<'UNIT'
[Unit]
Description=Explain a missing graphics device instead of showing a black screen
# Only when there is genuinely no DRM device. On every normal machine this
# condition is false and the unit never runs.
ConditionPathExists=!/dev/dri
After=multi-user.target
# GDM crash-loops in this situation; wait long enough that its restarts have
# stopped scribbling on the console before writing the message.
[Service]
Type=oneshot
ExecStartPre=/bin/sleep 12
ExecStart=/usr/lib/teddyos/no-gpu-notice
RemainAfterExit=yes
[Install]
WantedBy=graphical.target
UNIT

install -Dm644 /dev/stdin config/includes.chroot/usr/share/applications/teddyos-setup.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=teddyOS Permissions
Comment=Choose what this machine may touch
Exec=teddyos-setup
Icon=preferences-system-privacy
Terminal=false
Categories=Settings;System;
DESKTOP

# --- the look, as image defaults ---------------------------------------
# Same dconf keys scripts/teddyos-look.sh writes to a running guest. They are
# defaults, not user writes, so anything here stays changeable in Settings.
install -Dm644 /dev/stdin config/includes.chroot/etc/dconf/profile/user <<'EOF'
user-db:user
system-db:local
EOF

mkdir -p config/includes.chroot/etc/dconf/db/local.d
"$HERE/render-dconf.sh" > config/includes.chroot/etc/dconf/db/local.d/00-teddyos

# --- chroot hooks -------------------------------------------------------
cat > config/hooks/live/0100-teddyos-theme.hook.chroot <<'HOOK'
#!/bin/sh
# WhiteSur is fetched here rather than vendored: it is ~200 MB of assets across
# three repositories, and carrying that in the OS repo to rebuild an ISO nobody
# has asked for yet is the wrong trade.
set -e
# Name the missing tool. live-build reports only "hook failed (exit non-zero)",
# so without this the failure mode is a 40-minute build that dies with no clue
# which of four programs was absent.
for tool in git sassc curl; do
  command -v "$tool" >/dev/null || { echo "ERROR: $tool missing from chroot" >&2; exit 1; }
done
export TERM=xterm-256color   # WhiteSur's installer drives setterm/tput and
                             # dies without it, having printed a success banner
export HOME=/root
# SUDO_USER leaks in from the `sudo lb build` that started this, and names a
# user who exists on the BUILD HOST but not inside the chroot. WhiteSur does
#   MY_USERNAME="${SUDO_USER:-...}"; MY_HOME=$(getent passwd "$MY_USERNAME" ...)
# so MY_HOME comes back empty and the script dies at exit 2 with no output —
# and it works perfectly when you test the same commands on a running system,
# because there the user does exist. Root exists in both.
unset SUDO_USER SUDO_UID SUDO_GID
export USER=root LOGNAME=root
cd /tmp
# Clean first. A hook that failed partway leaves the clones behind, and the
# retry then dies on "destination path already exists" — an error about the
# previous failure, not the current one, which is the most misleading thing a
# rebuild can tell you.
rm -rf /tmp/WhiteSur-gtk-theme /tmp/WhiteSur-icon-theme /tmp/WhiteSur-cursors /tmp/WhiteSur-wallpapers
for repo in WhiteSur-gtk-theme WhiteSur-icon-theme WhiteSur-cursors WhiteSur-wallpapers; do
  git clone --depth=1 -q "https://github.com/vinceliuice/$repo.git" || exit 1
done
mkdir -p /usr/share/themes /usr/share/icons /usr/share/backgrounds/teddyos

cd /tmp/WhiteSur-gtk-theme
./install.sh -c Light -c Dark -a normal -t default >/dev/null
cd /tmp/WhiteSur-icon-theme && ./install.sh -a >/dev/null
cd /tmp/WhiteSur-cursors && ./install.sh >/dev/null 2>&1 || true
cd /tmp/WhiteSur-wallpapers && cp -f 4k/*.jpg /usr/share/backgrounds/teddyos/ 2>/dev/null || \
  cp -f 1080p/*.jpg /usr/share/backgrounds/teddyos/

# libadwaita reads ONLY ~/.config/gtk-4.0, so a system theme leaves Files and
# Settings rendering stock Adwaita. /etc/skel seeds it for every account the
# installer creates, including the live user.
mkdir -p /etc/skel/.config/gtk-4.0
if [ -d /usr/share/themes/WhiteSur-Light/gtk-4.0 ]; then
  cp -r /usr/share/themes/WhiteSur-Light/gtk-4.0/. /etc/skel/.config/gtk-4.0/
fi

rm -rf /tmp/WhiteSur-*
HOOK

cat > config/hooks/live/0200-teddyos-extensions.hook.chroot <<'HOOK'
#!/bin/sh
# Just Perfection and Rounded Window Corners are not in the Debian archive, and
# they are the two doing the actual work — one removes shell chrome, the other
# rounds windows. Pinned to the shell in THIS image, because a zip built for
# another GNOME installs cleanly and then never loads.
set -e
for tool in curl unzip python3 glib-compile-schemas gnome-shell; do
  command -v "$tool" >/dev/null || { echo "ERROR: $tool missing from chroot" >&2; exit 1; }
done
SHELL_VER=$(gnome-shell --version | grep -oE '[0-9]+' | head -1)
for uuid in just-perfection-desktop@just-perfection rounded-window-corners@fxgn; do
  url=$(curl -fsS --max-time 30 \
    "https://extensions.gnome.org/extension-info/?uuid=$uuid&shell_version=$SHELL_VER" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin).get("download_url",""))') || url=""
  if [ -z "$url" ]; then
    echo "ERROR: no $uuid build for GNOME $SHELL_VER" >&2
    exit 1   # fail the build: an image whose desktop is half-configured is
             # worse than no image, and this is silent at runtime
  fi
  curl -fsSL --max-time 120 -o /tmp/e.zip "https://extensions.gnome.org$url"
  mkdir -p /usr/share/gnome-shell/extensions/"$uuid"
  ( cd /usr/share/gnome-shell/extensions/"$uuid" && unzip -oq /tmp/e.zip )
  # Extension schemas must be compiled into the system directory or the
  # extension loads and every setting silently reads back as default.
  if [ -d /usr/share/gnome-shell/extensions/"$uuid"/schemas ]; then
    cp /usr/share/gnome-shell/extensions/"$uuid"/schemas/*.gschema.xml /usr/share/glib-2.0/schemas/ 2>/dev/null || true
  fi
  rm -f /tmp/e.zip
done
glib-compile-schemas /usr/share/glib-2.0/schemas/
HOOK

cat > config/hooks/live/0300-teddyos-claude.hook.chroot <<'HOOK'
#!/bin/sh
# Node 22 from nodejs.org, then Claude Code.
#
# Debian trixie ships Node 20.19.2 and Claude Code requires >=22 — the install
# fails with EBADENGINE and then `npm ERR! Exit handler never called!`, which
# looks like an npm bug rather than a version floor.
#
# From the official tarball rather than NodeSource, deliberately. Adding an apt
# source to a public image means every machine anyone installs from it pulls
# packages from a third party forever, silently, because they downloaded an ISO
# once. A tarball in /usr/local is a decision that ends when the build ends.
set -e
for tool in curl python3 tar xz sha256sum; do
  command -v "$tool" >/dev/null || { echo "ERROR: $tool missing from chroot" >&2; exit 1; }
done

case "$(dpkg --print-architecture)" in
  arm64) NODE_ARCH=arm64 ;;
  amd64) NODE_ARCH=x64 ;;
  *) echo "ERROR: unsupported arch for Node tarball" >&2; exit 1 ;;
esac

NODE_VER=$(curl -fsSL --max-time 60 https://nodejs.org/dist/index.json \
  | python3 -c 'import sys,json
d=json.load(sys.stdin)
print(next(r["version"] for r in d if r["version"].startswith("v22.") and r.get("lts")))')
[ -n "$NODE_VER" ] || { echo "ERROR: could not resolve a Node 22 LTS version" >&2; exit 1; }
echo "node: $NODE_VER ($NODE_ARCH)"

TARBALL="node-$NODE_VER-linux-$NODE_ARCH.tar.xz"
curl -fsSL --max-time 300 -o /tmp/node.tar.xz "https://nodejs.org/dist/$NODE_VER/$TARBALL"
# Verify against the signed-release SHASUMS. An image built from a corrupted or
# substituted runtime is not something to discover from a stranger's bug report.
curl -fsSL --max-time 60 -o /tmp/SHASUMS256.txt "https://nodejs.org/dist/$NODE_VER/SHASUMS256.txt"
want=$(grep " $TARBALL\$" /tmp/SHASUMS256.txt | cut -d" " -f1)
got=$(sha256sum /tmp/node.tar.xz | cut -d" " -f1)
[ -n "$want" ] || { echo "ERROR: $TARBALL not listed in SHASUMS256.txt" >&2; exit 1; }
[ "$want" = "$got" ] || { echo "ERROR: node tarball checksum mismatch" >&2; exit 1; }

mkdir -p /usr/local/lib/nodejs
tar -xJf /tmp/node.tar.xz -C /usr/local/lib/nodejs
NODE_DIR="/usr/local/lib/nodejs/node-$NODE_VER-linux-$NODE_ARCH"
for b in node npm npx; do ln -sf "$NODE_DIR/bin/$b" /usr/local/bin/$b; done
rm -f /tmp/node.tar.xz /tmp/SHASUMS256.txt

export PATH="$NODE_DIR/bin:$PATH"
node --version

# NOT >/dev/null 2>&1. Every masked command in this file has cost an hour: the
# theme hook hid a TERM error, WhiteSur hid its own stderr, and this hid the
# EBADENGINE that explained the whole thing.
# AI tools for Search "work on …". Claude is the default; others give people a
# choice without hunting package names. Installs that fail must not kill the
# image — Claude is load-bearing, the rest are best-effort.
npm install -g --silent @anthropic-ai/claude-code 2>&1 | tail -20
npm install -g --silent @xai-official/grok 2>&1 | tail -10 || \
  echo "note: grok npm install failed (non-fatal)"
npm install -g --silent @openai/codex 2>&1 | tail -10 || \
  echo "note: codex npm install failed (non-fatal)"
npm install -g --silent @google/gemini-cli 2>&1 | tail -10 || \
  echo "note: gemini-cli npm install failed (non-fatal)"
npm install -g --silent @github/copilot 2>&1 | tail -10 || \
  echo "note: copilot npm install failed (non-fatal)"

# Google Antigravity CLI (`agy`). Official installer; non-fatal in chroot.
if curl -fsSL https://antigravity.google/cli/install.sh -o /tmp/agy-install.sh 2>/dev/null; then
  bash /tmp/agy-install.sh 2>&1 | tail -15 || \
    echo "note: antigravity install failed (non-fatal)"
  rm -f /tmp/agy-install.sh
  # Installer often drops into ~/.local; copy into image PATH for all users.
  for cand in /root/.local/bin/agy /usr/local/bin/agy; do
    if [ -x "$cand" ]; then
      install -Dm755 "$cand" /usr/local/bin/agy
      echo "agy: linked from $cand"
      break
    fi
  done
else
  echo "note: antigravity install script unreachable (non-fatal)"
fi

# Presence is the authoritative check; execution is not. `claude` ships as a
# native ELF binary now, and running it inside a chroot without /proc, /dev and
# /sys mounted dies with a bare "Aborted" — which looks exactly like a broken
# install and is purely an artefact of where it ran. live-build does mount
# those during a real build, so --version usually works; when it does not, that
# is not a reason to throw away the image.
PKG="$NODE_DIR/lib/node_modules/@anthropic-ai/claude-code"
[ -d "$PKG" ] || { echo "ERROR: claude-code package not installed" >&2; exit 1; }
[ -x "$NODE_DIR/bin/claude" ] || { echo "ERROR: claude binary missing" >&2; exit 1; }
ln -sf "$NODE_DIR/bin/claude" /usr/local/bin/claude
# Symlink optional tools when npm put them next to node.
for b in grok codex gemini copilot; do
  if [ -x "$NODE_DIR/bin/$b" ]; then
    ln -sf "$NODE_DIR/bin/$b" /usr/local/bin/$b
    echo "$b: linked"
  fi
done

if ver=$("$NODE_DIR/bin/claude" --version 2>/dev/null); then
  echo "claude-code: $ver"
else
  echo "note: claude installed but not runnable in the build chroot (expected)"
fi
HOOK

cat > config/hooks/live/0400-teddyos-dconf.hook.chroot <<'HOOK'
#!/bin/sh
set -e
dconf update

# One fewer tile in the quick-settings panel. "Power Mode" only exists because
# power-profiles-daemon is installed, and it is meaningless in a VM and close
# to it on most desktops — a control that offers Balanced/Performance for a
# machine whose power profile nobody is going to think about.
#
# Night Light and Dark Style are built into gnome-shell and cannot be removed
# without adding ANOTHER extension to hide them, which is spending complexity
# to buy simplicity. Not worth it; they at least do something.
apt-get remove -y --purge power-profiles-daemon >/dev/null 2>&1 || true

# gnome-remote-desktop is pulled in by gnome-core and ships a daemon that can
# serve this machine's screen over the network. It is removed on principle, not
# for tidiness: an OS whose first screen promises that nothing reaches your
# files or the network without you turning it on should not arrive with a
# remote-access service installed by default. Anyone who wants it can install
# it, which is the same deal every capability on that screen offers.
apt-get remove -y --purge gnome-remote-desktop >/dev/null 2>&1 || true

# gnome-tour is the "Welcome to Debian / Take the Tour" first-login popup.
# Nontechnical first boot should land on the desktop, not a distro greeter.
apt-get remove -y --purge gnome-tour >/dev/null 2>&1 || true
mkdir -p /etc/xdg/autostart
cat > /etc/xdg/autostart/org.gnome.Tour.desktop <<'TOUR'
[Desktop Entry]
Type=Application
Name=Tour
Exec=true
Hidden=true
NoDisplay=true
X-GNOME-Autostart-enabled=false
TOUR

# Settings has 25 panels. These two come off cleanly and neither belongs in a
# first-run desktop: gnome-user-share serves your files over the network (same
# argument as remote-desktop), malcontent-gui is parental controls.
apt-get remove -y --purge gnome-user-share malcontent-gui >/dev/null 2>&1 || true

# NOT gnome-online-accounts, however obviously it looks like clutter. A dry-run
# shows removing it takes gnome-core AND gdm3 with it — the whole desktop and
# the login manager. Check `apt-get remove --dry-run` before deleting anything
# that looks optional in a metapackage-heavy install.

# Hide the panels that serve a specialist and confuse everyone else. NoDisplay
# reliably removes them from the app grid and search; whether GNOME 48 also
# drops them from the Settings sidebar is version-dependent and unverified
# here, so treat this as reducing the places they appear, not all of them.
for panel in wacom wellbeing color multitasking; do
  f=/usr/share/applications/gnome-$panel-panel.desktop
  [ -f "$f" ] && ! grep -q NoDisplay "$f" && echo "NoDisplay=true" >> "$f"
done
true
HOOK

cat > config/hooks/live/0500-teddyos-assert.hook.chroot <<'HOOK'
#!/bin/sh
# Assert the pieces a live session cannot start without.
#
# Everything here was absent at some point in this image's history and none of
# it produced an error at build time — the ISO built, booted, and simply did
# not work. --apt-recommends false is the common cause: it is right for image
# size and wrong for anything a package only Recommends.
set -e
missing=""
for tool in user-setup teddyos-search teddyos-search-app teddyos-setup teddyos-log-collect chromium gnome-shell gdm3; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
[ -f /etc/systemd/journald.conf.d/teddyos.conf ] \
  || missing="$missing journald-teddyos.conf"
[ -f /usr/lib/teddyos/logutil.py ] \
  || missing="$missing logutil.py"
# live-config creates the user at boot; without its scripts the login prompt
# has nothing to offer.
[ -f /lib/live/config/0030-user-setup ] || missing="$missing live-config/0030-user-setup"
[ -f /lib/live/config/0080-gdm3 ]       || missing="$missing live-config/0080-gdm3"

# A session GDM can actually launch, of each kind.
#
# Wayland alone is not enough. On a machine with no DRM device GDM turns
# Wayland off by itself and then needs an X session; with none installed it
# aborts with "no session desktop files installed" and restarts forever behind a
# black screen. That shipped once already, and nothing at build time complained.
[ -n "$(ls /usr/share/wayland-sessions/*.desktop 2>/dev/null)" ] \
  || missing="$missing wayland-session"
[ -n "$(ls /usr/share/xsessions/*.desktop 2>/dev/null)" ] \
  || missing="$missing xsession(gnome-session-xsession)"
# And the X driver that needs no KMS, or the X session above cannot start on
# exactly the machines it exists for.
[ -f /usr/lib/xorg/modules/drivers/fbdev_drv.so ] \
  || missing="$missing xserver-xorg-video-fbdev"

if [ -n "$missing" ]; then
  echo "ERROR: image is missing:$missing" >&2
  exit 1
fi
echo "assert: live-session prerequisites present"
HOOK

cat > config/hooks/live/0550-teddyos-logs.hook.chroot <<'HOOK'
#!/bin/sh
# Turn on persistent journal capture and the daily snapshot timer.
set -e
# tmpfiles creates /var/log/teddyos/{app,snapshots} with the sticky app dir.
systemd-tmpfiles --create /usr/lib/tmpfiles.d/teddyos.conf 2>/dev/null || true
systemctl enable teddyos-log-collect.timer 2>/dev/null \
  || systemctl --root=/ enable teddyos-log-collect.timer 2>/dev/null \
  || true
systemctl enable teddyos-update.timer 2>/dev/null \
  || systemctl --root=/ enable teddyos-update.timer 2>/dev/null \
  || true
# In a live-build chroot systemctl enable sometimes only works via the
# wants/ symlink. Force the link so a chroot without a running systemd still
# ships the timer enabled.
mkdir -p /etc/systemd/system/timers.target.wants
ln -sfn /etc/systemd/system/teddyos-log-collect.timer \
  /etc/systemd/system/timers.target.wants/teddyos-log-collect.timer
ln -sfn /etc/systemd/system/teddyos-update.timer \
  /etc/systemd/system/timers.target.wants/teddyos-update.timer
echo "logs: journald persistent + teddyos-log-collect.timer enabled"
echo "updates: teddyos-update.timer enabled (installs only if software.auto_update granted)"
HOOK

# Only ours. The glob `config/hooks/live/*.hook.chroot` also matches the hooks
# live-build installs itself, which are root-owned — chmod fails on those and,
# under `set -e`, kills the build after twenty minutes of downloading.
cat > config/hooks/live/0600-teddyos-bootmenu.hook.binary <<'HOOK'
#!/bin/sh
# Rewrite the boot menu in words a person would use.
#
# A .hook.binary in config/hooks/live/, not at the top of config/hooks/:
# live-build only executes hooks found in the live/ and normal/ subdirectories,
# and a hook sitting above them is skipped in silence — the build succeeds and
# ships the stock menu.
#
# Binary rather than chroot because these entries are GENERATED by live-build
# (the template only carries @LINUX_LIVE@), so there is nothing to override at
# config time. The binary tree is the first place the real text exists.
#
#   Live system (arm64)                     -> Try teddyOS - nothing is written...
#   Live system (arm64 fail-safe mode)      -> Try teddyOS (safe graphics)
#   Start installer                         -> Install teddyOS
#   Start installer with speech synthesis   -> Install teddyOS (screen reader)
#   Advanced install options... / Utilities -> Other options / Hardware tools
#
# "Live system" is jargon for "run it without touching your disk", which is the
# most reassuring thing this screen can tell someone who has never booted a USB
# stick and is afraid of wrecking their laptop. Worth saying outright.
set -e
# live-build runs binary hooks with `cd binary` first (binary_hooks:51), so the
# tree is at boot/grub, NOT binary/boot/grub. Accept either, because getting it
# wrong costs a full rebuild to discover and the check is two lines.
if   [ -f boot/grub/grub.cfg ];        then GRUB=boot/grub
elif [ -f binary/boot/grub/grub.cfg ]; then GRUB=binary/boot/grub
else
  echo "ERROR: cannot find grub.cfg (cwd=$(pwd))" >&2
  ls -d boot binary 2>/dev/null >&2
  exit 1
fi

# The dots are escaped. Unescaped they are wildcards, and `...'` then consumes
# the closing quote as well — leaving `submenu 'Other options'' --hotkey=a`,
# which is a GRUB syntax error that stops the menu drawing at all. That is a
# worse outcome than the wording being wrong.
sed -i \
  -e 's|menuentry "Live system ([a-z0-9]* fail-safe mode)"|menuentry "Try teddyOS (safe graphics)"|' \
  -e 's|menuentry "Live system ([a-z0-9]*)"|menuentry "Try teddyOS - nothing is written to your disk"|' \
  -e "s|submenu 'Advanced install options \.\.\.'|submenu 'Other options'|" \
  -e "s|submenu 'Utilities\.\.\.'|submenu 'Hardware tools'|" \
  "$GRUB/grub.cfg"

if [ -f "$GRUB/install_start.cfg" ]; then
  # Speech first: 'Start installer' is a prefix of it, so the general pattern
  # would otherwise rename both and lose the distinction.
  sed -i \
    -e "s|menuentry 'Start installer with speech synthesis'|menuentry 'Install teddyOS (screen reader)'|" \
    -e "s|menuentry 'Start installer'|menuentry 'Install teddyOS'|" \
    "$GRUB/install_start.cfg"
fi

# The BIOS menu, which on amd64 is a second, separate set of entries. Absent on
# arm64 (EFI only), so this is skipped rather than asserted on.
ISOLINUX=""
if   [ -d isolinux ];        then ISOLINUX=isolinux
elif [ -d binary/isolinux ]; then ISOLINUX=binary/isolinux
fi
if [ -n "$ISOLINUX" ]; then
  # syslinux writes labels bare — `menu label Live system (amd64)` — with no
  # menuentry keyword and no quotes, so these patterns are deliberately looser
  # than the GRUB ones above. Speech before the general installer pattern, for
  # the same prefix reason as GRUB.
  # \^\? everywhere, because syslinux marks its keyboard accelerator with a
  # caret INSIDE the label — "Start ^installer", "with ^speech synthesis",
  # "^Advanced install options". Patterns written against the plain words match
  # some of those and not others, which is worse than matching none: the speech
  # entry failed while the generic one succeeded, leaving the nonsense
  # "Install teddyOS with ^speech synthesis" on the menu.
  #
  # Speech first, for the same prefix reason as GRUB.
  sed -i \
    -e 's|Live system (\([a-z0-9]*\) fail-safe mode)|Try teddyOS (safe graphics)|g' \
    -e 's|Live system (\([a-z0-9]*\))|Try teddyOS - nothing is written to your disk|g' \
    -e 's|Start \^\?installer with \^\?speech synthesis|Install teddyOS (screen reader)|g' \
    -e 's|Start \^\?installer|Install teddyOS|g' \
    -e 's|\^\?Advanced install options|Other options|g' \
    -e 's|\^\?Utilities|Hardware tools|g' \
    -e 's|^menu title .*|menu title teddyOS|' \
    "$ISOLINUX"/*.cfg
fi

# Assert. A sed whose pattern stops matching after a live-build update fails
# silently and ships the old menu, which is exactly how this hook shipped
# nothing the first time.
#
# $ISOLINUX is included so the assertion cannot pass while the BIOS menu is
# still stock. It was GRUB-only, which meant an amd64 build could print
# "boot menu: renamed", exit 0, and hand a legacy-BIOS PC the Debian menu.
for stale in 'Live system' 'Advanced install options' 'Start installer'; do
  # $ISOLINUX/*.cfg, not $ISOLINUX. Passing the directory greps the syslinux
  # binaries too, and libgpl.c32 contains the word "Utilities" — so the
  # assertion could never pass on amd64 no matter what the menus said.
  if grep -q "$stale" "$GRUB/grub.cfg" "$GRUB/install_start.cfg" \
       ${ISOLINUX:+$ISOLINUX/*.cfg} 2>/dev/null; then
    echo "ERROR: boot menu rename did not apply - still matches: $stale" >&2
    exit 1
  fi
done
# A syslinux timeout of 0 means "wait forever", not "boot now" — the opposite of
# the GRUB reading. Someone who walks away comes back to the same screen.
if [ -n "$ISOLINUX" ] && grep -qE '^timeout[[:space:]]+0[[:space:]]*$' "$ISOLINUX"/*.cfg; then
  echo "ERROR: BIOS menu still has 'timeout 0' - it will never boot on its own" >&2
  exit 1
fi
if grep -qE "submenu '[^']*''" "$GRUB/grub.cfg"; then
  echo "ERROR: submenu rename left a doubled quote (GRUB will not draw the menu)" >&2
  grep -nE "submenu '[^']*''" "$GRUB/grub.cfg" >&2
  exit 1
fi

echo "boot menu: renamed (EFI${ISOLINUX:+ and BIOS})"
grep -hE "^menuentry |^submenu " "$GRUB/grub.cfg" "$GRUB/install_start.cfg" 2>/dev/null \
  | sed 's/ {$//' | sed 's/^/    /'
if [ -n "$ISOLINUX" ]; then
  grep -hE "^[[:space:]]*menu label |^label " "$ISOLINUX"/*.cfg 2>/dev/null \
    | sed 's/^[[:space:]]*/    BIOS: /'
fi
HOOK
chmod +x config/hooks/live/0600-teddyos-bootmenu.hook.binary

cat > config/hooks/live/0700-teddyos-calamares.hook.chroot <<'HOOK'
#!/bin/sh
# Select the teddyOS branding. One line, edited in place, because settings.conf
# is a conffile owned by calamares-settings-debian: shipping our own copy would
# silently revert whatever else Debian changed there between releases.
set -e
CONF=/etc/calamares/settings.conf
[ -f "$CONF" ] || { echo "calamares settings.conf missing — skipping" >&2; exit 0; }
sed -i 's|^branding:.*|branding: teddyos|' "$CONF"
# Assert, because a sed that stops matching after a Calamares update fails
# silently and ships the Debian-branded installer again.
grep -q '^branding: teddyos$' "$CONF" || {
  echo "ERROR: calamares branding not switched to teddyos" >&2; exit 1; }
[ -f /etc/calamares/branding/teddyos/branding.desc ] || {
  echo "ERROR: teddyos branding.desc was not installed" >&2; exit 1; }
echo "calamares: branded teddyOS"
HOOK
chmod +x config/hooks/live/0700-teddyos-calamares.hook.chroot

cat > config/hooks/live/0800-teddyos-icons.hook.chroot <<'HOOK'
#!/bin/sh
# Ensure every teddyOS app icon is present and the hicolor cache is current.
#
# Web: we ship our own globe tile (teddyos-web.svg + PNGs). Chromium PNGs are
# an optional extra only when our raster is missing — never leave Web pointing
# at theme name `web-browser` (Safari compass under WhiteSur).
#
# WhatsApp / Install / Search must not depend on Papirus panel glyphs or
# symbolic theme names — they are full dock tiles.
set -e
need="teddyos-search teddyos-web teddyos-whatsapp teddyos-accounts teddyos-install teddyos-answers teddyos-claude"
missing=0
for name in $need; do
  if [ ! -f "/usr/share/icons/hicolor/scalable/apps/${name}.svg" ] \
     && [ ! -f "/usr/share/icons/hicolor/48x48/apps/${name}.png" ]; then
    echo "ERROR: missing icon $name in hicolor" >&2
    missing=1
  fi
done
# Raster fallbacks if the build host lacked rsvg-convert but the chroot has it.
if command -v rsvg-convert >/dev/null 2>&1; then
  for svg in /usr/share/icons/hicolor/scalable/apps/teddyos-*.svg; do
    [ -f "$svg" ] || continue
    base=$(basename "$svg" .svg)
    for size in 48 64 128 256; do
      out="/usr/share/icons/hicolor/${size}x${size}/apps/${base}.png"
      if [ ! -f "$out" ]; then
        mkdir -p "$(dirname "$out")"
        rsvg-convert -w "$size" -h "$size" -o "$out" "$svg"
      fi
    done
  done
fi
# Last resort for Web: copy chromium if still no web tile at all.
if [ ! -f /usr/share/icons/hicolor/scalable/apps/teddyos-web.svg ] \
   && [ ! -f /usr/share/icons/hicolor/48x48/apps/teddyos-web.png ]; then
  found=0
  for size in 16 24 32 48 64 128 256; do
    src="/usr/share/icons/hicolor/${size}x${size}/apps/chromium.png"
    if [ -f "$src" ]; then
      install -Dm644 "$src" "/usr/share/icons/hicolor/${size}x${size}/apps/teddyos-web.png"
      found=$((found + 1))
    fi
  done
  if [ "$found" -eq 0 ]; then
    echo "ERROR: teddyos-web missing and chromium has no hicolor icon" >&2
    missing=1
  else
    echo "web icon: fell back to chromium artwork at $found sizes"
  fi
fi
# Rebuild cache so the live session sees new names immediately.
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null \
    || gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null \
    || true
fi
# Desktop files must not ship with theme placeholders.
for pair in \
  "teddyos-search.desktop:teddyos-search" \
  "teddyos-web.desktop:teddyos-web" \
  "teddyos-whatsapp.desktop:teddyos-whatsapp" \
  "teddyos-install.desktop:teddyos-install" \
  "teddyos-answers.desktop:teddyos-answers" \
  "teddyos-accounts.desktop:teddyos-accounts" \
  "teddyos-perplexity.desktop:teddyos-perplexity" \
  "teddyos-devin.desktop:teddyos-devin" \
  "teddyos-replit.desktop:teddyos-replit" \
  "teddyos-claude.desktop:teddyos-claude"
do
  desk=${pair%%:*}
  icon=${pair##*:}
  path="/usr/share/applications/$desk"
  if [ -f "$path" ]; then
    sed -i "s|^Icon=.*|Icon=$icon|" "$path"
  fi
done
[ "$missing" -eq 0 ] || exit 1
echo "teddyos icons: ok"
HOOK
chmod +x config/hooks/live/0800-teddyos-icons.hook.chroot

chmod +x config/hooks/live/0*-teddyos-*.hook.chroot

# --- build --------------------------------------------------------------
echo ">>> lb build  (this is the long part; $JOBS cpus)"
# Full log to a file, tail to the terminal. Piping straight to `tail` buffers
# everything until the build ends, so a run that wedges at 80% looks identical
# to one that is working — for an hour.
BUILD_LOG="$OUT_DIR/build-$ARCH.log"
set +e
sudo lb build 2>&1 | tee "$BUILD_LOG" | grep -E "^(P:|E:)" | tail -200
rc=${PIPESTATUS[0]}
set -e
if [[ $rc -ne 0 ]]; then
  echo "error: lb build failed (exit $rc). Full log: $BUILD_LOG" >&2
  tail -30 "$BUILD_LOG" >&2
  exit $rc
fi

built="$(find "$BUILD" -maxdepth 1 -name 'live-image-*.hybrid.iso' | head -1)"
[[ -n "$built" ]] || { echo "error: no ISO produced — see the log above" >&2; exit 1; }

mv "$built" "$IMAGE"
( cd "$OUT_DIR" && sha256sum "$(basename "$IMAGE")" > "$(basename "$IMAGE").sha256" )

echo
echo ">>> $IMAGE"
echo "    $(du -h "$IMAGE" | awk '{print $1}')"
echo "    $(cat "$IMAGE.sha256")"
