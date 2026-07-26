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

## What you should see

**Display:** after `make utm-run`, the UTM window shows the light setup
assistant, then the home launcher (search field + tiles).

**Serial:** UTM does **not** have View → Serial. Default serial is a PTTY. Use:

```sh
utmctl attach os   # prints PTTY: /dev/ttysN  (attach itself is not wired yet)
cat /dev/ttysN     # then read the guest COM1 log
```

Start the VM first so you catch `os: hello from kernel` and later UI lines.

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

Every UTM refresh wires COM2 as **TcpServer** on `127.0.0.1:7420`.
`make utm-bridged` starts the host bridge in dial mode
(`OS_MCP_BRIDGE_CONNECT=tcp:…`) so it retries until the guest appears — same
topology as headless `make run-bridged` / `smoke-bridge`. QEMU's TcpClient mode
does not retry; that is why the bridge is always the client.

Serial should show `mouse: usb-tablet ready` then `mouse: ps2 ready`, then
`ui: setup welcome`. With the bridge up you should also see `mcp: bridge live`
early and `mcp: bridge live` when entering the Bridge setup step.

## Override

```sh
UTM_VM_NAME=os-dev make utm-run
```

## Still use QEMU for CI

`make test` / `scripts/smoke_common.py` stay headless and deterministic. UTM is for interactive desktop use only.
