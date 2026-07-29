---
title: Replacing macOS — route A, and what it costs
version: v01
date: 2026-07-27
status: current — Linux live daily-driver path on this tip
---

# Replacing macOS — route A, and what it costs

> **Tip note.** Route A (`linux/` live image) is the daily-driver path on
> `fix/bridge-prewarm` / this review branch. It does **not** restore MCP
> `host/bridge` connectors. See `STATUS.md` and `AGENTS.md`.

## The question this answers

"Use just teddyOS and nothing else." Taken literally against the `no_std`
kernel, the answer is no, and not by a small margin. That kernel has no heap
(`extern crate alloc` appears nowhere), no filesystem, no network stack, no
process model, no ELF loader. It draws a framebuffer and searches an index
compiled into the ISO. `thesis-os-doc-v01.md` already retired the claim that
made it special; this document picks up where that left off.

So "teddyOS replaces macOS" has to mean: **teddyOS is a Linux distribution** —
our shell, our agent, our permission model, on a kernel that already has the
drivers we are never going to write.

## Two routes, one taken now

| | what it is | status |
|---|---|---|
| **A** | full-screen aarch64 Linux on this Mac, under Apple's hypervisor | **built** |
| **C** | the same userland on dedicated bare-metal hardware | the real destination |

Route B — Asahi on this machine — is not costed here because M4 Max support is
unverified and probably absent. It should be checked before anyone plans on it;
if it is a no, bare metal on *this* Mac is not a route at any point.

A is not a compromise on the way to C. It is the only way to find out what C
has to contain, because the list of things that break is not knowable from a
design document — it is knowable from living in it for a week.

## What was built

```
scripts/teddyos-vm.sh          headless boot: ssh, apt, provisioning, CI
scripts/teddyos-provision.sh   GNOME 48, Chromium, Claude Code
scripts/teddyos-portable.sh    survive being moved between hypervisors
scripts/teddyos-apps.sh        WhatsApp / Gmail / teddysearch as real apps
scripts/teddyos-look.sh        the macOS-shaped desktop
scripts/teddyos-utm.sh         hand the disk to UTM, which is the GPU path
```

Debian 13 arm64, 8 CPU / 12 GB / 120 GB. Boots to systemd in **2.5 s** under
`hvf`. Verified in the guest: Chromium 150, Node 20.19.2, Claude Code 2.1.220,
GNOME Shell 48.7, `[drm] features: +virgl`.

## Why the display must go through UTM

Homebrew's qemu is built without OpenGL and without virglrenderer:

```
$ qemu-system-aarch64 -device help | grep virtio-gpu
virtio-gpu-pci                      # no -gl variant
$ qemu-system-aarch64 -display help
none curses cocoa dbus              # no gl-capable backend
```

A desktop on it renders through llvmpipe on the CPU. UTM 4.7.5 bundles
`virglrenderer.1.framework`, `epoxy.0.framework` and `GLESv2.framework`, and
its aarch64 build does have `virtio-gpu-gl-pci` and `virtio-ramfb-gl`. Same
disk, same hypervisor — the difference is entirely who draws.

`teddyos-vm.sh` is not obsoleted by this. It stays the scriptable path.

## Failures that look like something else

Five of these cost real time, and every one of them presents as a different
problem than it is. They are the reason this document exists.

**1. UTM drops a VM with a malformed config, silently and completely.** A
`Network` entry missing `MacAddress` or `IsolateFromHost` fails to decode, and
a failed decode discards the whole VM — gone from `utmctl list`, absent from
Console, nothing in `log show`. The bundle and the registry row both survive,
so it reads as a UTM bug. Found only by bisecting the patch one block at a time.

**2. UTM's "Shared" mode ignores PortForward.** It is `-netdev vmnet-shared`,
not QEMU user-mode networking, so the guest gets its own address on a host
bridge and the forwarding table is never consulted. `cat /var/db/dhcpd_leases`
for the real IP; ssh there.

**3. cloud-init pins the network to the NIC's MAC.** Debian's cloud image writes
a netplan config matching by hardware address. Move the disk to another
hypervisor and the match fails: the desktop comes up perfectly with no network,
which reads as a broken image. `teddyos-portable.sh` matches on `Name=en*`.

**4. `ssh host 'cmd'` inside a `while read` loop eats the loop's stdin.** One
iteration runs, the rest vanish, exit status 0. Always `ssh -n`.

**5. WhiteSur's installer needs `TERM`.** It drives the cursor with `setterm`
and `tput`, and redirects stderr to a log it deletes on exit. Over
non-interactive ssh it prints its banner, installs nothing, exits 1 — which
looks exactly like "this theme does not support GNOME 48".

**6. A GTK theme does not theme libadwaita.** `install.sh` populates `~/.themes`
and everything looks right — until you open Files or Settings, which are
libadwaita and read only `~/.config/gtk-4.0`. That directory stays empty unless
you pass `-l`, so modern GNOME apps silently render stock Adwaita: the desktop
is themed and half the windows are not. It presents as "this theme is
low-quality", not as a missing flag.

The shape they share: **a failure that reports success, or reports the wrong
cause.** Every verification step in these scripts exists because of one of them.

## The look

"Like macOS" is not a setting, and the first pass proved that skinning alone is
not enough either — GNOME with a macOS theme still reads as GNOME. What made the
difference was subtraction, not more theme.

**Removed** (Just Perfection): the Activities button, the app menu, the overview
dash, the workspace popup, the clock's empty world-clock and weather cards, and
the app-grid tile in the dock. Boot goes straight to the desktop instead of the
overview, and the hot corner is off — a pointer that wanders into a corner and
throws the whole screen into an overview is the opposite of calm.

**Kept and tuned**: a bottom dock that hides and magnifies, five apps and no
controls; window controls left in close/min/max order; Inter at 12 rather than
11, because the cheapest way to look more expensive is more air around the type;
rounded window corners; a Monterey gradient rather than Sonoma's saturated
sweep, so the dock and the type are what you notice.

Two settings are counter-intuitive and should not be "fixed" by a later reader:

- **The panel blur is off.** Blurring it samples the wallpaper behind it, so on
  any gradient the bar becomes a muddy smear that changes colour along its own
  length. Unblurred, it is simply transparent — which is what the macOS menu bar
  actually looks like.
- **The dock's background colour is set explicitly to white.** Left to itself it
  inherits the shell theme, and WhiteSur's shell CSS is dark, which produced a
  near-black slab under a light wallpaper.

Type is **Inter**, not SF Pro: Apple licenses SF Pro for use on Apple platforms
and it is not ours to install here.

Settings are written to `/etc/dconf` as *defaults*, never as writes to the
user's database — so every one is overridable in Settings and nothing fights a
change.

## Known limits — none of these have a fix at this layer

- **No Google Chrome.** Google ships Linux Chrome for amd64 only. Chromium is
  the same engine minus the proprietary bits; the one that bites is Widevine,
  so DRM video does not play. This is an argument for x86-64 in route C.
- **Chromium is not GPU-accelerated.** virgl does not expose the ES 3.0 context
  Chromium asks for: `eglCreateContext ES 3.0 failed with EGL_BAD_ATTRIBUTE`.
  The desktop is accelerated; the browser composites on the CPU.
- **iMessage cannot leave macOS.** The only path is a BlueBubbles server on a
  Mac with Full Disk Access. Under route A the Mac is the host, so this is
  fine. Under route C a Mac must stay powered on forever. Deferred.
- **Autologin is on**, which is why the web apps pass `--password-store=basic`:
  a keyring that was never unlocked prompts on first launch. Turning autologin
  off restores normal keyring behaviour and is the better posture.
