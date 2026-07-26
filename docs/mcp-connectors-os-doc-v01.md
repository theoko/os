---
project: os
type: architecture
purpose: MCP-style connectors (email first) via host bridge
status: active
---

# MCP connectors (v01)

## Principle

Connectors (**email**, search, skills, …) are **not** linked into the kernel.
They are MCP-shaped tools behind **capabilities**. Until the guest has a network
stack, the guest talks to a **host bridge** on the Mac over COM2 (QEMU TCP serial).

```
UI / agent  --(cap: email.search)-->  guest MCP client (COM2 TcpServer :7420)
                                            |
                              host bridge dials tcp:127.0.0.1:7420
                                            |
                              gog gmail / mock / skills / search indexes
```

One topology everywhere: the guest listens; the bridge dials and retries
(`make run-bridged` / `utm-bridged`). Secrets stay on the host (`gog` keyring).
Nothing secret enters `os/` or the ISO.

## Wire protocol (line-oriented, ASCII)

Guest → host:

| Request | Meaning |
|---------|---------|
| `PING` | Liveness |
| `CALL email.search q=<gmail query> max=<n>` | Search mail |
| `CALL email.send …` | Always `ERR … disabled_until_cap_confirm` (policy stub) |
| `CALL search.query q=<keywords> k=<n> …` | Knowledge search (+ optional email/files/audio scopes) |
| `CALL skills.list` / `skills.save` | Skill playbooks |
| `CALL doc.read url=…` | Open a result body |
| `CALL workspace.index` / `audio.transcribe` / `*.forget` | Host indexes + revoke |

Host → guest:

| Response | Meaning |
|----------|---------|
| `OK pong` | Alive |
| `OK email.search` / `ROW from=…\|subj=…` / `END` | Mail hits |
| `OK search.query` / `ROW title=…\|…` / `END` | Knowledge hits |
| `ERR <tool> <reason>` | Failure |

Fields use `key=value`; use `|` between fields. Values are single-line; spaces allowed after `=`.

## Capabilities (guest)

`Cap::EmailSearch` (chosen at setup / Caps screen) is required before
`CALL email.search`. Ambient root is forbidden for arbitrary tools.

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_EMAIL_BACKEND=mock` (default) | Deterministic fake rows (CI) |
| `OS_MCP_EMAIL_BACKEND=gog` | `gog gmail search -j --results-only …` via keyring |

Search always uses the in-bridge corpus + teddy/files/mail when those caps are
granted on the `CALL` (see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md)).

## Bridge API

Also see skills in [`docs/skills-os-doc-v01.md`](skills-os-doc-v01.md) and search in [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).

## Commands

```sh
make bridge          # build host/bridge
make bridge-run      # foreground listen :7420 (debug / nc)
make run-bridged     # guest COM2 listens; host bridge dials
make utm-bridged     # same topology under UTM
```
