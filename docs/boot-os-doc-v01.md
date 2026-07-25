---
project: os
type: boot
purpose: Limine + QEMU boot notes
status: active
---

# Boot — Limine + QEMU (v01)

## Stack

- **Bootloader:** Limine (`v9.x-binary` branch, fetched by `make` into `./limine/`)
- **Protocol:** Limine (not Multiboot)
- **Kernel:** ELF64 higher-half at `0xffffffff80000000`, entry `kmain`
- **Host run:** `qemu-system-x86_64 -M q35` with BIOS CD boot (no OVMF required for smoke)

## Config

[`limine.conf`](../limine.conf) lives on the ISO at `/boot/limine/limine.conf`:

- `protocol: limine`
- `path: boot():/boot/kernel`

## Limine requests (kernel)

Placed in `.requests` (see `kernel/linker-x86_64.ld`):

- `BaseRevision`
- `StackSizeRequest` (128 KiB)
- `HhdmRequest` / `MemoryMapRequest` (USB tablet DMA pages use the memory map)

## Serial + smoke exit

- Early printk: COM1 (`0x3F8`) → QEMU `-serial stdio` or `-serial file:…`
- Banner: `os: hello from kernel`
- Success signal: write `0x10` to QEMU `isa-debug-exit` at `0xf4` → host exit status **33**

## Commands

```sh
make build   # ISO
make run     # interactive serial
make smoke   # non-interactive check (scripts/smoke-qemu.sh)
```

## Linker / rustflags

Workspace [`.cargo/config.toml`](../.cargo/config.toml) sets for `x86_64-unknown-none`:

- `-C relocation-model=static` (must be `ET_EXEC`, not PIE — Limine will not load a PIE)
- `-C code-model=kernel`

`kernel/build.rs` passes an absolute path to `linker-x86_64.ld`.

## Apple Silicon note

Cross-compile from `aarch64-apple-darwin` to `x86_64-unknown-none`. QEMU runs the x86_64 guest under emulation; expect slower boots than on native x86_64 hosts.
