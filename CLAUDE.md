---
project: os
purpose: identity card for agents and humans
status: active
updated: 2026-07-31
version_note: Runtime / ship version is the top-level VERSION file (not Cargo package versions).
---

# os

Agent-centric hobby operating system in Rust (`no_std` kernel), targeting
**x86_64 and ARM64**, booted with **Limine**. Headless x86 runs use **QEMU**;
desktop runs use **UTM** (x86) or **VirtualBox** (native ARM64 on Apple Silicon).
The daily-driver path is a **Linux** live image under `linux/`.

When you land here: read this file first, then `VERSION` + the newest
`CHANGELOG.md` entry, then touch only the layer you need.

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

**Audience: non-technical people first.** If a message needs a command, a flag,
or a wire name to understand, it is wrong for the default UI. Advanced mode and
logs may be precise; Guided mode and every empty state must be human.

## NON-NEGOTIABLES

1. **Non-technical first.** Plain language on every user-facing surface. No
   `gh` / SSH / package jargon on screen. One clear action beats a howto.
2. **QEMU for smoke tests.** `make test` = kernel unit tests + QEMU serial smoke.
3. **Standalone only.** All host-connector queries return Offline. No COM2 bridge.
4. **Capability model.** Agents operate under kernel caps. Email, search, files,
   diagnostics stay denied until a cap is granted.
5. **No privileged inference.** All computation runs in userspace (future). Skills are markdown.
6. **Tests gate commits.** Run `make test` (or the touched Linux contract subset)
   before committing. Bug fixes land **with** a test when practical.
7. **Version + changelog on real work.** When behavior ships, bump top-level
   `VERSION` and add a newest-first entry to `CHANGELOG.md` (YAML frontmatter
   `version: vX.Y.Z`, `project: os`, `updated:`, `type: changelog`). Semver-ish:
   minor = feature, patch = fix. Conventional commits preferred
   (`feat(scope):`, `fix(scope):`, `docs:`, `release: vX.Y.Z`).
8. **The corpus ships inside the ISO.** Search is compiled in, so `search/corpus.json`
   at build time *is* what the image knows — titles plus 320 characters each. The
   default bake is personal (the workspace index), and `make publish-os` refuses
   to upload it. Publish from `./scripts/bake-corpus.py --seed-only`, then re-bake.
9. **No secrets in the tree.** Never bake tokens into the ISO.

## Copy checklist (before shipping UI)

- Would a parent / non-developer friend understand this in one read?
- Is the next step a button, not a command to memorize?
- Are failures explained as consequences, not errno / exit codes?
- Does Advanced (if any) hold the technical names — not the default path?

## Two product lines (do not confuse them)

| Target | Artifact | What you see |
|--------|----------|--------------|
| Freestanding kernel | `os.iso` / `os-arm64.iso` | “hello” setup, local caps, offline corpus |
| Linux daily driver | `teddyos-<VERSION>-<arch>-<build>.iso` via `make linux-iso` | GNOME + Search, setup, agents, updates |

`make virtualbox-arm64` → freestanding ARM kernel only. Linux UX requires the live ISO.

## Ship a change

1. Implement + run the relevant tests (`make test` and/or `scripts/linux-contract-unit.sh`)
2. Bump `VERSION` + add a newest-first `CHANGELOG.md` entry; sync frontmatter
3. Conventional commit (`feat:`, `fix:`, `docs:`, or `release: vX.Y.Z` when the bump is the point)
4. Rebuild the ISO that actually carries the change (kernel vs `linux-iso`)

## Common commands

```sh
make build              # freestanding x86 ISO (standalone)
make run                # QEMU
make iso-arm64          # freestanding native ARM64 ISO
make virtualbox-arm64   # VirtualBox ARM freestanding VM
make linux-iso          # Debian live daily-driver (needs guest build env)
make test               # host + QEMU smoke
./scripts/bake-corpus.py
```

## Conventions

- Docs: `docs/<topic>-os-doc-vNN.md` with YAML frontmatter (project/purpose/status)
- Do not restructure the tree “for cleanliness” without an explicit ask
- Multi-session: check `git status` before large deletes or force-pushes
