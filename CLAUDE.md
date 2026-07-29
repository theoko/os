---
project: os
purpose: identity card for agents and humans
status: active
---

# os

Agent-centric hobby operating system in Rust (`no_std` kernel), targeting
**x86_64 and ARM64**, booted with **Limine**. Headless x86 runs use **QEMU**;
desktop runs use **UTM** (x86) or **VirtualBox** (native ARM64 on Apple Silicon).
The daily-driver path is a **Linux** live image under `linux/`.

## Shape

```
kernel/          freestanding Rust kernel (Limine + framebuffer UI, standalone)
core/            shared types and capsule system
linux/           live Debian image + teddyOS desktop apps
search/          seed.json (curated) + corpus.json (generated, baked in)
skills/defaults/ local agent skill playbooks
docs/            versioned design notes
scripts/         QEMU / UTM / VirtualBox / publish helpers
Makefile
```

North star: **capability-based kernel**, **fully standalone**.
No host bridge, no external MCP connectors. Everything the freestanding OS does
runs locally.

## NON-NEGOTIABLES

1. **QEMU for smoke tests.** `make test` = kernel unit tests + QEMU serial smoke.
2. **Standalone only.** All host-connector queries return Offline. No COM2 bridge.
3. **Capability model.** Agents operate under kernel caps. Email, search, files
   stay denied until a cap is granted.
4. **No privileged inference.** All computation runs in userspace (future). Skills are markdown.
5. **Tests gate commits.** Run `make test` before committing.
6. **The corpus ships inside the ISO.** Search is compiled in, so `search/corpus.json`
   at build time *is* what the image knows — titles plus 320 characters each. The
   default bake is personal (the workspace index), and `make publish-os` refuses
   to upload it. Publish from `./scripts/bake-corpus.py --seed-only`, then re-bake.
7. **No secrets in the tree.** Never bake tokens into the ISO.

## Common commands

```sh
make build              # ISO (standalone)
make run                # QEMU
make iso-arm64          # native ARM64 ISO
make virtualbox-arm64   # VirtualBox ARM VM
make test               # host + QEMU smoke
./scripts/bake-corpus.py
```
