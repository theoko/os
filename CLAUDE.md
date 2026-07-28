---
project: os
purpose: identity card for agents and humans
status: active
---

# os

Agent-centric hobby operating system in Rust (`no_std` kernel), targeting **x86_64**,
booted with **Limine**. Headless runs use **QEMU**; the desktop GUI front-end is **UTM**
(Parallels cannot run x86_64 guests on Apple Silicon).

## Shape

```
kernel/          freestanding Rust kernel (Limine + framebuffer UI, no bridges)
core/            shared types and capsule system
search/          seed.json (curated, public) + corpus.json (generated, baked in)
skills/defaults/ local agent skill playbooks
docs/            versioned design notes
scripts/         QEMU smoke tests
Makefile
```

North star: **capability-based kernel**, now **fully standalone**.
No host bridge, no external MCP connectors. Everything the OS does runs locally.

## NON-NEGOTIABLES

1. **QEMU for smoke tests.** `make test` = kernel unit tests + QEMU serial smoke.
2. **Standalone only.** All queries return Offline. No bridge probing on COM2.
3. **Capability model.** Agents operate under kernel caps. Email, search, files stay denied until cap granted.
4. **No privileged inference.** All computation runs in userspace (future). Skills are markdown.
5. **Tests gate commits.** Run `make test` before committing.
6. **The corpus ships inside the ISO.** Search is compiled in, so `search/corpus.json`
   at build time *is* what the image knows — titles plus 320 characters each. The
   default bake is personal (the workspace index), and `make publish-os` refuses
   to upload it. Publish from `./scripts/bake-corpus.py --seed-only`, then re-bake.

## Common commands

```sh
make build           # ISO (now standalone-only)
make run             # QEMU (no bridge needed)
make test            # host + QEMU smoke, in both feature configurations
./scripts/bake-corpus.py   # rebuild search/corpus.json, then rebuild the ISO
```

