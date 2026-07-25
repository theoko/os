---
title: Substrate and architecture — two separate decisions
version: v02
supersedes: linux-os-doc-v01.md
date: 2026-07-25
status: proposal — not started
---

# Substrate and architecture — two separate decisions

v01 got the diagnosis wrong, in a way that would have been expensive.

It attributed the performance ceiling to the custom kernel and proposed Linux
as the fix. The ceiling is real, but the cause is not the kernel.

## Measured

The guest runs `-accel tcg`: every instruction is translated in software,
because the image is x86-64 and the host is arm64.

```
qemu-system-aarch64 -accel help  ->  hvf, tcg
```

`hvf` is Apple's Hypervisor.framework. An aarch64 guest was launched under it
on this machine and booted — hardware virtualisation is available and works.

So the frame numbers from v01:

| operation | cycles | wall | ceiling |
|---|---|---|---|
| full-screen present | 67,468k | ~22.5 ms | ~44 fps |
| dirty-rect present | 103k | ~0.034 ms | far past 60 fps |

are a property of **emulating a foreign architecture**, not of writing our own
kernel.

## Two axes, not one

| change | buys |
|---|---|
| x86-64 → **aarch64** | the performance ceiling |
| custom kernel → **Linux** | running real applications |

v01 bundled these. They are independent.

**The trap this creates:** migrating to *x86-64* Linux would keep TCG and keep
the 22 ms frames. All of the porting cost, none of the performance benefit.
Anyone reading v01 and starting work could reasonably have done that. If the
substrate moves, it must move to aarch64.

## The option v01 never considered

Porting the existing kernel to `aarch64-unknown-none` fixes graphics with no
Linux at all, and keeps enforcement-by-construction — the property that makes
the capability claim true today.

It is not free. On `virt` there is no port I/O, no PS/2, no PIT, so:

| file | today | on aarch64 |
|---|---|---|
| `mouse.rs`, `usb_tablet.rs` | PS/2 + UHCI | virtio-input |
| `pci.rs` | port-I/O CAM | ECAM/MMIO |
| `beep.rs` | PIT channel 2 | virtio-sound, or drop |
| `serial.rs` | 16550 UART | PL011 |
| `fb.rs` | Limine framebuffer | unchanged — still a linear framebuffer |

`caps`, `search`, `skills`, `font`, `anim`, `ui`, `searchui`, `screens`, and
the whole bridge are architecture-independent and would not change.

That is a fraction of the Linux port, and it is reversible.

## What this does not change

The LibreOffice argument stands. A custom kernel cannot run real applications
in any realistic timeframe, on any architecture, and everything v01 says about
the thesis — that "no ambient root" does not survive arbitrary binaries, and
that real sandboxing becomes the only honest option — is unaffected.

## Revised recommendation

Decide the two questions separately, in this order:

1. **Do you need real applications?** If yes, Linux, and the enforcement
   question from v01 has to be answered first. If no, the kernel can stay.
2. **Whichever substrate: go aarch64.** This is not optional and not a detail.
   It is where the performance actually comes from, and it is cheap relative to
   everything else.

If the answer to (1) is "yes but not yet", porting the current kernel to
aarch64 is a defensible intermediate step: it makes the thing pleasant to use
now, costs a fraction of the full move, and throws away nothing that the Linux
port would have kept anyway.

---

*v01 is retained below for the inventory and the thesis discussion, both of
which stand.*


---
title: Moving to a Linux substrate
version: v01
date: 2026-07-25
status: proposal — not started
---

# Moving to a Linux substrate

## The decisive argument: real applications

Someone will want to open a document in LibreOffice or Word.

A custom kernel cannot do that in any realistic timeframe. It needs an ELF
loader, POSIX syscalls, a filesystem, glibc, a display server, fontconfig,
printing — years of work to arrive at something Linux already is. The
performance argument below is about *cost*. This one is about *possibility*,
and it is the one that settles the question.

It also reframes the project, and that deserves stating plainly rather than
being discovered later.

## What running real apps does to the thesis

LibreOffice with filesystem access **is** ambient authority. There is no
version of "no ambient root" that survives running arbitrary desktop binaries
under the old definition. If the OS runs real applications, the current claim
is simply false, however the UI is worded.

The coherent reframe — and it is a better product than the current one:

> **A desktop where every application's access is explicit, and the agent is
> one of those applications.**

The capability screen stops being an agent-only consent flow and becomes the
system's permission manager. `workspace.index`, `audio.transcribe`,
`portal.sync` are already exactly the right shape for that; they just apply to
one process today. Extending them to every app is a generalisation, not a
rewrite.

That makes **option 2 below (real sandboxing) the only honest choice**, because
the enforcement now has to hold against binaries nobody in this repo wrote.
Flatpak and bubblewrap already do the containment; what is missing from that
world is a coherent, legible consent surface — which is the part this project
has actually built and got right.

## The case



A day of work on this repo went into: framebuffer pixel formats, PS/2 versus
UHCI device enumeration, font rasterisation, Retina upscaling filters, dirty
rectangles, and PIT timing. Every one of those is a solved problem that we
solved again by hand.

The performance ceiling is not a bug that can be fixed in our draw path. UTM
runs x86-64 under **TCG software emulation** on Apple Silicon — no hardware
virtualisation, no GPU. Measured on a real boot:

| operation | cycles | wall | ceiling |
|---|---|---|---|
| full-screen present | 67,468k | ~22.5 ms | ~44 fps |
| dirty-rect present | 103k | ~0.034 ms | far past 60 fps |

Dirty rectangles bought a 655x improvement on incremental frames, and
interaction is now fine. But a full repaint is a megabyte over emulated MMIO,
and that number does not move without a GPU.

**The kernel was the interesting part to build. It is now the expensive part
to own.**

## What is actually differentiated

The value of this project is not the kernel. It is:

- the capability model and the consent flow that chooses it
- the knowledge graph and multi-source search
- the connectors, and the rule that connectors run on the host

None of that requires writing a UHCI driver.

## Inventory

Kernel is ~6,900 lines. Split by what a move would do to it:

**Deleted — solved by the platform (~2,700 lines)**

| file | lines | replaced by |
|---|---|---|
| `fb.rs` | 731 | DRM/KMS, or any toolkit |
| `usb_tablet.rs` | 674 | evdev |
| `mouse.rs` | 418 | evdev |
| `keyboard.rs` | 291 | evdev |
| `pci.rs` | 242 | the kernel |
| `beep.rs` | 178 | ALSA/PipeWire |
| `font.rs` + generated atlas | 152 + 284 KB | fontconfig / any text stack |

**Rewritten — same behaviour, different substrate (~2,000 lines)**

`ui.rs`, `searchui.rs`, `screens.rs`, `setup.rs`, `anim.rs`. The *layouts and
copy* survive; the drawing calls do not. The build-time font atlas, the
integer-only AA (no FPU), and the fixed-point easing all exist because of
constraints that disappear.

**Ports close to unchanged (~1,100 lines)**

`caps.rs`, `search.rs`, `skills.rs`, `mcp.rs` — the capability bitset, the
fixed-point index, the builtin skills, the line protocol. `search.rs` could
even keep its integer scoring, though it would no longer need to.

**Untouched (~3,400 lines)**

The entire `host/bridge`: email graph, workspace index, teddy, portals,
transcription, `doc.read`. It talks a line protocol over a socket and does not
care what is on the other end.

So: roughly **2,700 lines deleted, 2,000 rewritten, 4,500 kept**.

## The hard question: what happens to the thesis

`AGENTS.md` says capability-scoped, no ambient root. Today that is enforced
because the kernel has no other way to do anything — there is no filesystem, no
network, no syscall surface to escape through. That is a strong claim, and it
is strong *because* the kernel is small.

On Linux the claim weakens by default. A process that can `open()` anything is
not capability-scoped no matter what our UI says.

Three honest options:

1. **Agent shell as PID 1.** Minimal userspace, our shell is the only thing
   with access, connectors still on the host over a socket. Keeps most of the
   claim; the kernel is now trusted infrastructure rather than the enforcement
   point.
2. **Sandbox the agent** (seccomp, namespaces, landlock). Enforcement becomes
   real and auditable, but it is now Linux's model rather than ours, and the
   capability list must map onto it honestly rather than decoratively.
3. **Accept it as a convention.** Fastest, and the weakest claim. If this is
   chosen, `AGENTS.md` should be amended to say so rather than continuing to
   assert something the code no longer enforces.

**Given the requirement to run real applications, option 2 is the answer.**
Options 1 and 3 both assume the only thing running is code we wrote. Once
LibreOffice is in scope, enforcement has to be real and it has to come from the
kernel, because the alternative is a permission screen that describes
restrictions nothing applies.

## What is gained

- Graphics, input, audio, and **networking** — email and portals reachable
  in-guest, which means the COM2 bridge becomes optional rather than load-bearing
- GPU-accelerated compositing: 60 fps is a configuration, not a research project
- A text stack, so no build-time atlas and no ASCII-only copy (the `0x20..=0x7E`
  restriction that shaped every string in the UI disappears)
- Suspend, resume, multiple displays, hotplug — none of which we have

## What is lost

- The demo value of "this is a kernel that boots to an agent"
- Enforcement-by-construction, unless option 1 or 2 is taken seriously
- ~6,900 lines of working, tested code, roughly 40% of it deleted outright
- The constraint that produced good decisions: no FPU forced fixed-point
  scoring, no allocator forced bounded buffers, and both are better for it

## Suggested sequence

Nothing here is started. In order:

1. **Settle the enforcement question** above. Everything else follows from it.
2. **Prove the substrate**: minimal Linux image, our shell, bridge over vsock
   or a unix socket. Measure frame times before porting UI.
3. **Port the logic** — `caps`, `search`, `skills` — with their tests. These
   should pass unchanged.
4. **Rebuild the UI** against a real toolkit, reusing layouts and copy.
5. **Delete the drivers** last, once the replacement demonstrably works.

Keep `main` bootable throughout. The current kernel is a working artifact and
should stay that way until the replacement is better, not merely newer.

## Recommendation

Do it, on **option 2**, and rewrite `AGENTS.md` first.

The requirement to run real applications settles the substrate question — a
custom kernel cannot get there. It also retires the current phrasing of the
thesis: "no ambient root" cannot survive running arbitrary binaries, and the
document should say what the system will actually enforce rather than what the
old kernel made true by accident.

The replacement claim is stronger, not weaker: every application's access is
explicit, the agent included. That is a product people would want on a machine
that also runs Word — and it is the part of this repo worth keeping.
