---
project: os
type: architecture
purpose: MCP-style connectors (email first) via host bridge
status: active
---

# MCP connectors (v01)

## Principle

Connectors (**email**, calendar, drive, …) are **not** linked into the kernel.
They are MCP-shaped tools behind **capabilities**. Until the guest has a network
stack, the guest talks to a **host bridge** on the Mac over COM2 (QEMU TCP serial).

```
UI / agent  --(cap: email.search)-->  guest MCP client (COM2)
                                            |
                                     QEMU -serial tcp:7420
                                            |
                                      host/bridge
                                            |
                              gog gmail / mock / future MCP servers
```

Secrets stay on the host (`gog` keyring under Application Support). Nothing
secret enters `os/` or the ISO.

## Wire protocol (line-oriented, ASCII)

Guest → host:

| Request | Meaning |
|---------|---------|
| `PING` | Liveness |
| `LIST` | Tool names |
| `CALL email.search q=<gmail query> max=<n>` | Search mail |
| `CALL email.send to=<addr> subj=<s> body=<b>` | Send (cap-gated; bridge may require confirm) |
| `CALL search.query q=<keywords> k=<n> cat=<opt>` | Knowledge search (curated corpus) |

Host → guest:

| Response | Meaning |
|----------|---------|
| `OK pong` | Alive |
| `OK tools=a,b,c` | Tool list |
| `OK email.search n=<N>` / `ROW from=…\|subj=…` / `END` | Mail hits |
| `OK search.query n=<N> backend=…` / `ROW title=…\|…` / `END` | Knowledge hits |
| `ERR <tool> <reason>` | Failure |

Fields use `key=value`; use `|` between fields. Values are single-line; spaces allowed after `=`.

## Capabilities (guest)

Guest `Caps` (setup + live switches) gate COM2 calls. The bridge also checks
wire flags so a forged CALL cannot bypass consent:

| Cap | Wire bit | Tools |
|-----|----------|-------|
| `email.search` | (guest refuse) / `email=1` on search | inbox + email graph |
| `search.query` | guest refuse | `search.query` |
| `workspace.index` | `files=1` | workspace index / file docs |
| `audio.transcribe` | `audio=1` | transcripts |
| `skills.save` | `skills=1` | `skills.save` |
| `portal.sync` | `portal=1` | `tsearch.sync`, `teddy.*`, `market.*` |

Ambient root is forbidden: a missing grant is a hard deny.

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_EMAIL_BACKEND=mock` (default) | Deterministic fake rows (CI) |
| `OS_MCP_EMAIL_BACKEND=gog` | `gog gmail search -j --results-only …` via keyring |
| `OS_MCP_SEARCH_BACKEND=tfidf` (default) | Curated `search/corpus.json` (tSearch-style) |
| `OS_MCP_SEARCH_BACKEND=mock` / `tsearch` | Demo rows / live tsearch-revival |

## Bridge API

Also see skills in [`docs/skills-os-doc-v01.md`](skills-os-doc-v01.md) and search in [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).

## Commands

```sh
make bridge          # build host/bridge
make bridge-run      # listen :7420 (mock)
make run-bridged     # QEMU COM1 stdio + COM2 → bridge (gog if set)
```
