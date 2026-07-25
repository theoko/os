---
project: os
type: run
purpose: boot the hobby OS in UTM (Parallels-like GUI on Apple Silicon)
status: active
---

# UTM — desktop GUI front-end (v01)

## Why UTM, not Parallels

On Apple Silicon, **Parallels / VMware only run ARM guests**. This kernel is **x86_64**, so those apps cannot boot `os.iso`.

**UTM** is a QEMU GUI that *can* emulate x86_64 (TCG). Same ISO as `make run`, with a windowed console.

## Prerequisites

```sh
brew install --cask utm   # already used by `make utm` if present
make iso                  # produces os.iso
```

## Commands

```sh
make utm           # build ISO + create/refresh UTM VM named "os"
make utm-run       # same, then start the VM
make utm-bridged   # ensure host MCP bridge is up, wire COM2, start VM
```

The VM is created via UTM’s AppleScript API:

- backend: QEMU
- architecture: `x86_64`
- `hypervisor: false` (TCG — required on Apple Silicon)
- `uefi: true` (GOP framebuffer for the home UI)
- memory: 1024 MiB
- removable drive: repo `os.iso`

## Seeing the hello banner

**Display (easiest):** after `make utm-run`, the UTM window should show
`os: hello from kernel` on a dark background (Limine framebuffer text).

**Serial:** UTM does **not** have View → Serial. Default serial is a PTTY. Use:

```sh
utmctl attach os   # prints PTTY: /dev/ttysN  (attach itself is not wired yet)
cat /dev/ttysN     # then read the guest COM1 log
```

That shows the guest serial. (Start the VM first; if you
attach after the Phase 1 halt you may miss the line — the framebuffer stays.)

Optional GUI serial: VM settings (✏️) → Devices → New → Serial → Mode:
**Built-in Terminal**.

## Mouse cursor

UTM/SPICE feeds an absolute **usb-tablet**. The guest drives it with a minimal
**UHCI + HID** stack (`kernel/src/usb_tablet.rs`).

`make utm` turns UTM's built-in USB input **off** and adds a dedicated
`piix3-usb-uhci` + `usb-tablet` via `QEMU.AdditionalArguments` (flat string
argv tokens — dict-shaped entries make UTM drop the VM from the library).
PS/2 stays on for keyboards. Move the Mac pointer over the guest window — no
capture required for the tablet.

If a prior run left **ghost** library entries (name `os` registered but the
`.utm` bundle was deleted), `make utm` scrubs them from UTM's preferences and
recreates once. Subsequent runs only refresh the ISO + config in place.

### Live connectors (COM2 → host bridge)

`make utm-bridged` starts the host bridge on `127.0.0.1:7420` and adds a second
UTM **Serial** device in `TcpClient` mode (COM2) pointing at that address —
first-class UTM config, not a raw `-serial` flag. Finish setup, click
**Connectors** for a live `search.query` hit in the footer.

Serial should show `mouse: usb-tablet ready` then `mouse: ps2 ready`, then
`ui: setup welcome`. With the bridge up you should also see `mcp: email connected`
early and `mcp: bridge live` when entering the Bridge setup step.

## Override

```sh
UTM_VM_NAME=os-dev make utm-run
```

## Still use QEMU for CI

`make test` / `scripts/smoke-qemu.sh` stay headless and deterministic. UTM is for interactive desktop use only.
