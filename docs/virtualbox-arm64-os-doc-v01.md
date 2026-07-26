---
title: VirtualBox ARM64
version: 1
status: active
---

# VirtualBox ARM64

The ARM64 edition runs natively in VirtualBox on Apple Silicon. It is a
separate image from the x86-64 ISO:

```sh
make virtualbox-arm64
```

That command builds `os-arm64.iso`, creates or refreshes the
`teddyOS-arm64` VM, selects VirtualBox's ARM chipset and QemuRamFB display,
enables the USB 1.1 (OHCI) keyboard/tablet path, attaches the fresh ISO, and
opens the VM.

## Why the settings are specific

VirtualBox on an ARM host cannot run the x86-64 image. The ARM VM must use:

- `armv8virtual` chipset and EFI firmware;
- `qemuramfb` graphics at 1280×800;
- USB keyboard and USB tablet;
- OHCI on, EHCI/xHCI off.

The last line is essential. VirtualBox connects its built-in input devices to
the selected USB host controller. This kernel has a native polled OHCI driver;
putting those devices behind EHCI or xHCI makes the desktop render correctly
but leaves it unable to receive input.

## What is smooth now

- The kernel composites into cached RAM and sends only dirty rectangles to
  QemuRamFB.
- Idle frames perform no framebuffer MMIO.
- ARM timing uses `CNTVCT_EL0`/`CNTFRQ_EL0`, so transitions have real
  millisecond pacing rather than CPU-dependent spin counts.
- Keyboard and pointer reports are polled every millisecond without waiting
  for an interrupt controller.
- VirtualBox's absolute tablet is decoded using its native eight-byte report
  layout. The OHCI done queue is consumed and acknowledged on every completed
  transfer, so pointer delivery remains continuous.
- Cursor moves save and restore the pixels beneath the arrow, preventing
  trails or a second cursor at the initial center position.
- The ARM64 target builds the optimized release kernel by default. Its MMIO
  helpers deliberately emit simple non-writeback loads and stores because
  VirtualBox's ARM interpreter can stall on equivalent pre/post-indexed forms.
- Debug UART output is muted on VirtualBox after platform detection because
  that emulated PL011 can retain a full FIFO and must never pace the UI.

This gives the custom desktop responsive typing, pointer tracking, and short
transitions. It does not yet provide guest-driven dynamic resolution when the
host window is resized. QemuRamFB is a firmware framebuffer; macOS/Windows-like
live resize requires a display driver and a mode-change protocol (for example,
virtio-gpu), not a faster blit loop.

## Manual build

To produce only the ISO:

```sh
make iso-arm64
```

To launch an existing VM headlessly for diagnostics:

```sh
VBOX_FRONTEND=headless make virtualbox-arm64
```
