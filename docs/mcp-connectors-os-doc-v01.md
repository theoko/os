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
| `CALL email.search [q=…]` | Inbox peek count (`ROW n=`); guest omits args; gog reports true match count |
| `CALL email.send …` | Always `ERR … disabled_until_cap_confirm` (policy stub) |
| `CALL search.query q=<keywords> [k=n] …` | Knowledge search (+ optional files/audio scopes) |
| `CALL skills.list` / `skills.save` | Skill playbooks |
| `CALL doc.read url=…` | Open a result body |
| `CALL workspace.index files=1` | Force-rebuild file index (nc); guest `files=1` search lazy-builds when empty |
| `CALL audio.transcribe path=… audio=1` | Host-only: index one media file (guest never sends `path=`) |
| `CALL workspace.forget` / `audio.forget` | Revoke purge |

Host → guest:

| Response | Meaning |
|----------|---------|
| `OK pong` | Alive |
| `OK email.search` / `ROW n=<count>` / `END` | Mail peek count only (no message fields on the wire) |
| `OK search.query` / `ROW title=…\|…` / `END` | Knowledge hits |
| `ERR <tool> <reason>` | Failure |

Fields use `key=value`; use `|` between fields. Values are single-line; spaces allowed after `=`.

## Capabilities (guest)

`Cap::EmailSearch` (chosen at setup / Caps screen) is required before
`CALL email.search`. Ambient root is forbidden for arbitrary tools.

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_EMAIL_BACKEND=mock` (default) | Deterministic `ROW n=` peek count (CI) |
| `OS_MCP_EMAIL_BACKEND=gog` | `gog gmail search -j --results-only …` via keyring |

Search uses the in-bridge corpus + teddy/files/audio when those scopes are
granted on the `CALL` (see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md)).
Files lazy-builds; audio needs a prior host `audio.transcribe`. Mail stays
peek-only via `email.search`.

## Bridge API

Also see skills in [`docs/skills-os-doc-v01.md`](skills-os-doc-v01.md) and search in [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).

## Commands

```sh
make bridge          # build host/bridge
make bridge-run      # foreground listen :7420 (debug / nc)
make run-bridged     # guest COM2 listens; host bridge dials
make utm-bridged     # same topology under UTM
```
