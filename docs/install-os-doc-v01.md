---
title: Running on real hardware
version: v01
date: 2026-07-25
status: current — freestanding ISO path; see tip note
---

# Running on real hardware

> **Tip note.** This tip is **standalone**: there is no Live host MCP bridge
> over COM2. Bare-metal boots still matter for native-speed freestanding ISO
> demos; daily-driver on Apple Silicon is the Linux live path (`linux/`,
> VirtualBox ARM64). See `AGENTS.md` / `STATUS.md`.

## Why this document exists

On an Apple Silicon Mac, nothing can virtualise an x86-64 guest. This is not a
configuration problem:

```
VBOX_E_PLATFORM_ARCH_NOT_SUPPORTED (0x80bb0012)
Cannot run the machine because its platform architecture x86 is not supported on ARM.
```

VirtualBox reports `Supported platform architectures: ARMv8`. Parallels is the
same. Both virtualise — they hand guest instructions to the host CPU, and an
ARM CPU cannot execute x86.

UTM works because QEMU **emulates**: every guest instruction is translated in
software (TCG). That is why it runs at all, and equally why it will never be
smooth. Measured on a real boot, a full-screen present costs ~22.5ms, a hard
ceiling around 44fps, and no dirty-rectangle work moves it.

So there are two honest ways to see this OS at full speed: boot real x86-64
hardware, or port the kernel to aarch64. This document covers the first.

## Writing the stick

```
make usb-list                 # what is plugged in
DEVICE=/dev/diskN make usb    # write it
```

`os.iso` is a hybrid image: Limine installs both a BIOS stage and an EFI boot
path, with a protective MBR and a GPT. The same stick boots legacy and UEFI
machines.

The script writes to a raw block device, so it is deliberately hard to misfire:

- refuses anything reported as internal
- refuses partitions (`/dev/disk4s1`) — whole disks only
- refuses a disk holding the running system volume
- no autodetect; `DEVICE` must be named
- makes you retype the device path before it writes
- re-reads the stick afterwards and compares SHA-256 against the ISO

A stick that writes "successfully" but holds different bytes is the worst
outcome — it fails on another machine with no clue why — so the verify step is
not optional.

## Booting

Boot menu is usually **F12**, **F2**, or **Del**.

**Secure Boot must be off.** The Limine loader is not signed, and a machine
with Secure Boot enabled will refuse it without a useful message.

Nothing is written to the target machine's disks. The ISO is read-only and this
OS has no filesystem driver — it cannot persist anything, including the
capability choices made at setup. Every boot starts the journey fresh.

## What actually works on real hardware

Honest status, because the gap between "it boots" and "you can use it" is
where the surprise lives.

| | QEMU/UTM | real hardware |
|---|---|---|
| boot (BIOS and UEFI) | yes | yes |
| framebuffer | yes | yes, via UEFI GOP |
| keyboard | yes (i8042) | **only if the machine has an i8042** |
| pointer | yes (UHCI tablet) | **unlikely — see below** |
| host MCP bridge over COM2 | not on this tip (standalone) | N/A — no Live bridge |

### The input problem

This kernel drives two input paths: the PS/2 controller (i8042) and a USB
tablet on a **UHCI** host controller. UHCI is a USB 1.1 controller. QEMU
provides one because the VM config asks for it. Machines built in roughly the
last fifteen years ship **xHCI** instead, and many laptops route the built-in
keyboard over USB or I2C-HID with no i8042 anywhere.

So a modern laptop can boot this to a correct, fully rendered home screen with
a cursor that never moves and a keyboard that does nothing. That is
indistinguishable from a hang, which is why the OS now checks and says so: it
surveys the PCI bus for USB controllers and reports what it found rather than
pretending.

If you see

> No usable keyboard or mouse. This machine's USB is xHCI, which this OS cannot
> drive yet.

then the boot succeeded and the input drivers are the gap. An older desktop
with PS/2 ports, or any machine exposing a UHCI/EHCI controller, will be
driveable. Writing an xHCI driver is the work that changes this.

### Connectors (standalone)

On this tip there is **no** host MCP bridge. Search answers from the corpus
baked into the ISO. Mail, host files, and live portals are Offline stubs —
same on QEMU and bare metal. Giving the guest a network stack (or restoring a
bridge product) is separate work and is **not** implied by booting a USB stick.

## Recommendation

Boot real hardware to see the freestanding OS run at native speed and to prove
the boot path end to end. Expect input to be the thing that decides whether a
given machine is usable, and check the status line on the home screen first —
it now tells you which half is missing.

For day-to-day product work on this tip, prefer the **Linux live** path under
`linux/` (VirtualBox ARM64 / UTM as documented). The freestanding ISO remains
the QEMU smoke artifact; it is not the daily-driver surface.
