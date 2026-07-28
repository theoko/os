---
title: VirtualBox and the live Debian image
version: 1
status: active
---

# VirtualBox and the live Debian image

This is about `teddyos-<version>-<arch>-<buildid>.iso`, the Debian 13 GNOME live
image. It is **not** about the `no_std` kernel ISO — that has its own page,
[VirtualBox ARM64](virtualbox-arm64-os-doc-v01.md), and the two behave nothing
alike. Confusing them costs an afternoon; they even want opposite storage
controllers.

## Status, in one line

On Apple Silicon the live image **boots on VirtualBox but never draws a picture**.
Use UTM instead, which is proven working. On x86 VirtualBox this has not yet been
tested, and nothing below predicts it — the x86 device model is entirely
different and mature.

## The one thing that will actually bite you

A VM that has **both** an AHCI controller and a virtio-scsi controller hard-locks
the guest a fraction of a second after the boot menu. VirtualBox says so in its
own log, and this is the single most useful line in this document:

```
00000000e0420000..e0421fff (VIRTIOSCSI0) conflicts with existing mapping
00000000e0420000..e0421fff (AHCI)
VERR_IOM_MMIO_RANGE_CONFLICT
```

The firmware assigns PCI addresses one way; Linux re-enumerates and reassigns
BARs during early boot and hands virtio-scsi an address AHCI already holds.
VirtualBox refuses the mapping and the guest wedges — all vCPUs frozen on one
instruction, no disk I/O, nothing on screen.

It is easy to create this state by accident, because the natural fix for "the DVD
will not boot from AHCI" is to *add* a virtio-scsi controller, and adding one
does not remove the other:

```sh
VBoxManage storagectl "<vm>" --name SATA --remove
```

Remove the controller you stopped using. Attaching the ISO elsewhere is only half
the job.

## Why virtio-scsi at all

VirtualBox's ARM EFI will not boot a DVD attached to the Intel AHCI controller —
it never enumerates it as bootable, so you get firmware and then nothing. On
virtio-scsi the same ISO boots straight to the teddyOS menu. So the live image
wants virtio-scsi *only*, and the empty AHCI controller must go.

## What still does not work, and what was ruled out

With the collision removed the guest genuinely runs: the NIC comes up about
fifteen seconds in and completes a DHCP exchange, and network traffic keeps
ticking over afterwards. It simply never programs a display. After GRUB sets
800×600, VirtualBox logs no further mode change for the rest of the boot.

Three things were tried and did not help:

- **A serial console in the image.** The firmware and GRUB reach the port, so you
  do get the UEFI banner and GRUB's module errors. The kernel never does.
  VirtualBox's ARM machine gives Linux no ACPI SPCR or DT entry describing that
  UART, so `console=ttyS0` matches no device and is silently dropped. You cannot
  see in.
- **`VBoxInternal2/EfiGraphicsResolution`.** Set it to match GRUB's 800×600 so no
  mode switch happens mid-boot; VirtualBox ignored it and still came up at
  1280×800.
- **Changing kernel parameters from the host.** `VBoxManage controlvm
  keyboardputscancode` does not reach this VM's USB HID keyboard. Three Down
  presses at the boot menu and the highlight never moved, the countdown never
  cancelled. You cannot type in either.

Between those two, the machine can be neither observed nor steered from the host,
which is why this is parked rather than solved.

## A diagnostic that does generalise

Because an afternoon went into guessing at a black rectangle, the live image now
carries a serial console on every build:

```
console=tty0 console=ttyS0,115200n8 console=ttyAMA0,115200n8
```

`tty0` is first on purpose. The **last** console named becomes `/dev/console`, but
every console named still receives the kernel log — so a laptop keeps its screen
and a VM can also be handed a log. Both serial device names appear because they
are not the same device on the two architectures we ship: x86 has the 16550 at
`ttyS0`, while aarch64 virtual machines (QEMU, UTM) expose a PL011 at `ttyAMA0`.
Naming one covers half the builds, and the half it misses is the half that boots
to a black screen. A console that does not exist on a given machine is ignored,
so naming both costs nothing.

Point any hypervisor's serial port at a file and a silent boot becomes a log:

```sh
VBoxManage modifyvm "<vm>" --uart1 0x3f8 4 --uart-mode1 file /tmp/guest.log
```

This works everywhere except, as above, VirtualBox on ARM.

## Telling "hung" from "running but blind"

Worth writing down, because the obvious test is wrong.

**Do not** use `VBoxManage controlvm acpipowerbutton` as a liveness probe on ARM.
VirtualBox's ARM machine has no wired power button, so a healthy guest ignores it
exactly like a dead one.

What actually distinguishes them:

| probe | hung | alive but blind |
|---|---|---|
| `debugvm getregisters --cpu=N pc` | identical PC on every CPU | also identical — it's the idle loop |
| `debugvm statistics` disk `BytesRead` | flat | flat once booted |
| `debugvm statistics` NIC `TransmitBytes` | zero | **grows** |
| `Logs/VBox.log` for `AssertLogRel` | present | absent |

The network counter and the hypervisor's own assertions are the reliable pair.
An identical program counter across all vCPUs looks alarming and means nothing —
idle CPUs all park at the same instruction.
