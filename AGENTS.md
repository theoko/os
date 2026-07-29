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
kernel/          freestanding Rust kernel (Limine + framebuffer UI + COM2 MCP client)
host/bridge/     host MCP connector bridge (email, skills, search)
search/          curated knowledge corpus ({t,u,c,b,pr} — tSearch-style)
skills/defaults/ agent skill playbooks
docs/            versioned design notes
scripts/         QEMU / bridge smoke
Makefile
```

North star: **capability-based agents**. MCP connectors (email, search, …) run on the **host bridge**,
not in the kernel. Guest holds caps and calls tools over COM2 until a guest network stack exists.

## NON-NEGOTIABLES

1. **QEMU first for CI.** `make test` = host unit tests + QEMU serial smoke + MCP bridge smoke.
2. **No secrets in the tree.** Gmail OAuth lives in the `gog` keyring. Saved skills live in
   `~/Library/Application Support/os/skills/` — never bake tokens into the ISO.
3. **Capability model.** Personal-data and write CALLs need Caps / wire bits —
   no ambient root on those paths. Catalog peeks (`skills.list` / `skills.get`)
   are ungated. `email.send` needs the Send mail Cap **and** Brief Confirm
   (`confirm=1`); never auto-CALL send.
4. **Inference and HTTP stay out of the kernel.** Bridge + future userspace only. Skills are markdown playbooks, not privileged code.
5. **Tests gate commits.** Run `make test` before committing.

## Common commands

```sh
make build           # ISO
make run             # QEMU (MCP offline unless COM2 wired)
make bridge-run      # host bridge on :7420 (EMAIL_BACKEND=mock|gog)
make run-bridged     # QEMU COM2 → bridge
make test
```

Skills: defaults in `skills/defaults/`; save via bridge `CALL skills.save name=…` → Application Support.
