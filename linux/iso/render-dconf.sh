#!/usr/bin/env bash
# Emit the teddyOS desktop defaults as a dconf keyfile, on stdout.
#
# One file, two consumers: linux/iso/build-iso.sh bakes the output into the
# image, and scripts/teddyos-look.sh pipes it into a running guest. They were
# briefly two copies of the same eighty lines, which is a guarantee that the
# ISO and the machine you tested on will disagree about something small and
# you will find out from a screenshot.
#
# Values are overridable by environment so the live tuner can pass whatever it
# actually found installed, rather than what the image is known to contain.
set -euo pipefail

GTK_THEME="${GTK_THEME:-WhiteSur-Light}"
ICON_THEME="${ICON_THEME:-WhiteSur-light}"
CURSOR_THEME="${CURSOR_THEME:-WhiteSur-cursors}"
WALL="${WALL:-/usr/share/backgrounds/teddyos/Monterey-light.jpg}"
WALL_DARK="${WALL_DARK:-/usr/share/backgrounds/teddyos/Monterey-dark.jpg}"
# Search leads. It is the one thing this OS is for, and until now it had no
# icon at all — the feature existed only as a terminal command, which on a
# machine whose premise is "ask it things" made the premise invisible.
# No Terminal. A ">_" tile means nothing to most people, and of everything in
# the dock it is the one that looks like you could break the computer with it.
# It stays installed and is one search away for anyone who wants it — being in
# the dock is a recommendation, and recommending a shell to someone who has
# never seen one is not a kindness.
# teddyos-install last, and only meaningful while running from the USB stick or
# ISO: someone who has decided they like this needs to be able to keep it
# without rebooting and knowing to choose a different boot entry. It is removed
# from the dock automatically once the system is installed — see the
# teddyos-dock-install autostart, which is the thing that makes putting it here
# safe rather than permanent clutter.
FAVORITES="${FAVORITES:-'teddyos-search.desktop', 'teddyos-claude.desktop', 'teddyos-web.desktop', 'teddyos-whatsapp.desktop', 'org.gnome.Nautilus.desktop', 'teddyos-install.desktop'}"

cat <<EOF
[org/gnome/desktop/interface]
gtk-theme='$GTK_THEME'
icon-theme='$ICON_THEME'
cursor-theme='$CURSOR_THEME'
# 12, not 11. Debian's default is sized for a dense desktop; macOS is not
# dense, and the cheapest way to look more expensive is air around the type.
font-name='Inter 12'
document-font-name='Inter 12'
monospace-font-name='monospace 12'
color-scheme='default'
accent-color='blue'
# Off. A pointer that wanders into a corner and throws the whole screen into an
# overview fires by accident far more often than on purpose.
enable-hot-corners=false
# Just the time. "Tue Jul 28 14:52" is four pieces of information in a bar
# whose entire job is to stay out of the way, and three of them are things you
# already know. The date is one click away in the calendar drop-down.
clock-show-weekday=false
clock-show-date=false
clock-show-seconds=false
font-antialiasing='rgba'
font-hinting='slight'

[org/gnome/desktop/wm/preferences]
button-layout='close,minimize,maximize:'
titlebar-font='Inter Semi Bold 12'

[org/gnome/shell]
enabled-extensions=['dash-to-dock@micxgx.gmail.com', 'blur-my-shell@aunetx', 'user-theme@gnome-shell-extensions.gcampax.github.com', 'just-perfection-desktop@just-perfection', 'rounded-window-corners@fxgn', 'light-style@gnome-shell-extensions.gcampax.github.com', 'appindicatorsupport@rgcjonas.gmail.com', 'teddyos@teddysearch.com']
favorite-apps=[$FAVORITES]

[org/gnome/shell/extensions/user-theme]
name='$GTK_THEME'

# --- the part that makes it simpler -----------------------------------------
# Everything else here restyles GNOME. This removes it.
[org/gnome/shell/extensions/just-perfection]
# Boot to the desktop. GNOME starts in the overview, so the first thing anyone
# sees on first boot is a workspace picker rather than the machine.
startup-status=0
activities-button=false
app-menu=false
dash=false
workspace-popup=false
workspaces-in-app-grid=false
# The clock's dropdown ships a world clock and a weather card, both empty and
# both configured elsewhere. An empty panel is fine; one that opens onto two
# unfilled cards is not.
world-clock=false
weather=false
events-button=false
window-menu-take-screenshot-button=false
panel-size=32
animation=2

[org/gnome/shell/extensions/dash-to-dock]
dock-position='BOTTOM'
extend-height=false
dock-fixed=false
autohide=true
intellihide=true
intellihide-mode='FOCUS_APPLICATION_WINDOWS'
show-apps-at-top=false
# The app-grid tile is the one icon in the dock that is not an app. The grid is
# still a keystroke away.
show-show-apps-button=false
apply-custom-theme=false
custom-theme-shrink=true
# FIXED, not DYNAMIC: dynamic transparency re-tints the dock whenever a window
# nears it, so it is never the same colour twice and the eye keeps checking.
transparency-mode='FIXED'
# White, explicitly. Left alone the dock inherits the shell theme, and
# WhiteSur's shell CSS is dark — which puts a near-black slab under a light
# wallpaper, the least macOS-looking thing on the screen.
custom-background-color=true
background-color='#ffffff'
background-opacity=0.45
dash-max-icon-size=56
icon-size-fixed=false
animate-show-apps=false
show-mounts=false
show-trash=false
click-action='minimize-or-previews'
scroll-action='cycle-windows'
running-indicator-style='DOTS'
disable-overview-on-startup=true
hot-keys=false

[org/gnome/shell/extensions/blur-my-shell/panel]
# Off, and this is the counter-intuitive one. Blurring the panel samples the
# wallpaper behind it, so on any gradient the bar becomes a smear that changes
# colour along its own length. Unblurred it is simply transparent, which is
# what the macOS menu bar actually looks like.
blur=false

# All blur off, everywhere. Two reasons, and the second is the real one:
#
#   It is the most expensive thing the shell does. virgl gives the guest GL,
#   but every blurred frame is still a full-surface readback and gaussian pass
#   on an emulated GPU — it is what makes window drags and overview feel
#   syrupy, which reads as "this OS is slow" rather than "this effect is".
#
#   And it was never buying much. A frosted panel over a gradient wallpaper is
#   a smear; a frosted dock at 45% white looks near-identical without the blur.
#   Paying frame time for an effect nobody can point at is the worst trade in
#   a UI.
[org/gnome/shell/extensions/blur-my-shell/overview]
blur=false

[org/gnome/shell/extensions/blur-my-shell/dash-to-dock]
blur=false

[org/gnome/shell/extensions/blur-my-shell/appfolder]
blur=false

[org/gnome/shell/extensions/blur-my-shell/lockscreen]
blur=false

[org/gnome/shell/extensions/blur-my-shell/window-list]
blur=false

[org/gnome/shell/extensions/rounded-window-corners-reborn]
global-radius=12
skip-libadwaita-app=false
skip-libhandy-app=false

[org/gnome/mutter]
edge-tiling=true
dynamic-workspaces=true
center-new-windows=true

[org/gnome/desktop/peripherals/touchpad]
natural-scroll=true
tap-to-click=true

[org/gnome/desktop/background]
picture-uri='file://$WALL'
picture-uri-dark='file://$WALL_DARK'
picture-options='zoom'

[org/gnome/desktop/screensaver]
picture-uri='file://$WALL'
EOF
