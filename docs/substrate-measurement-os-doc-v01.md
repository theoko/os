---
title: What actually costs us 22.5ms a frame
version: v01
date: 2026-07-26
status: current — corrects the diagnosis in linux-os-doc-v02.md
---

# What actually costs us 22.5ms a frame

## Why measure

`linux-os-doc-v02.md` argues for a Linux substrate partly on performance, and
attributes the frame ceiling to **TCG instruction emulation**: a full-screen
present costs 67,468k cycles, about 22.5ms, a hard ceiling near 44fps.

That diagnosis is mostly wrong, and the correction matters because it changes
what a migration would actually buy.

Measured with `scripts/substrate-bench.sh` on this machine: the same Alpine
aarch64 image, same devices, same benchmark, only the accelerator changed.

## Numbers

| what | throughput |
|---|---|
| bulk RAM copy, HVF (hardware virtualisation) | **21.2 GB/s** |
| bulk RAM copy, TCG (instruction emulation) | **3.2 GB/s** |
| our full-screen present, TCG (4.1MB in 22.5ms) | **0.18 GB/s** |

```
536870912 bytes (512.0MB) copied, 0.023565 seconds, 21.2GB/s   # hvf
134217728 bytes (128.0MB) copied, 0.039198 seconds,  3.2GB/s   # tcg
```

## What that decomposes into

Instruction emulation costs **~6.6x** (21.2 → 3.2 GB/s). Real, but nothing
like the whole story — a memory copy is few instructions per byte, which is
the case TCG handles best.

The remaining **~18x** (3.2 → 0.18 GB/s) is not the accelerator at all. It is
the framebuffer itself. We write pixels into an emulated VGA device's MMIO
aperture, and every access traps to the device model. Bulk RAM under the *same*
accelerator moves seventeen times faster than our framebuffer does.

So roughly:

- ~6.6x from emulating x86-64 instructions
- ~18x from writing through trapped MMIO instead of memory

Boot time, incidentally, shows only a 2x difference (8s HVF vs 16s TCG) —
booting is dominated by device probing and waiting, so it understates the tax
badly. It is the wrong benchmark for this question and was discarded.

## What this changes

The migration case gets **stronger**, but for a different reason than the doc
gives. Under a Linux substrate with `virtio-gpu`, the framebuffer is ordinary
guest memory that the host maps — there is no per-write trap. A 4.1MB frame at
HVF memory speed is about **0.2ms** rather than 22.5ms, roughly 115x. Most of
that win comes from deleting the MMIO trap, not from the accelerator.

It also means the honest phrasing of the ceiling is *"we present frames through
a trapped MMIO aperture"*, not *"TCG is slow"*. Two consequences:

1. **Porting to aarch64 alone would not fix this.** Hardware virtualisation
   buys the 6.6x. If the guest still writes into a trapped framebuffer
   aperture, the 18x stays. The substrate has to bring a framebuffer that is
   plain memory, which is what virtio-gpu and a real display stack give.
2. **Dirty rectangles were the right call and remain so.** They avoid the
   expensive path rather than making it cheaper — which is exactly why they
   bought 655x on incremental frames while the full-repaint number did not
   move.

## How to reproduce

```
make linux-vm            # once, to fetch the Alpine image
./scripts/substrate-bench.sh            # boot-time comparison (weak, kept for context)
./scripts/substrate-bench.sh --memcpy   # the one that answers the question
```

Both write logs under `.linux-vm/`.

## Caveat

The 0.18 GB/s figure is our own x86 guest's full-screen present, measured
earlier in this project; the 21.2 and 3.2 figures are an aarch64 Alpine guest.
Different guests and different architectures, so the decomposition is
indicative rather than exact. What is not in doubt: framebuffer MMIO is an
order of magnitude worse than RAM under the same accelerator, and that is the
dominant term.
