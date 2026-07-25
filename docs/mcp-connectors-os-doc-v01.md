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
| `CALL email.search q=<gmail query> max=<n> email=1` | Search mail (needs `email=1`) |
| `CALL calendar.list email=1` | List upcoming events (`id=` + same `email=1` bit) |
| `CALL doc.read url=cal://… email=1` | Open a calendar event body |
| `CALL email.forget` | Delete the host mail knowledge graph |
| `CALL workspace.index files=1` | Build the project-folder index |
| `CALL workspace.forget` | Delete the workspace index |
| `CALL doc.read url=… [files=1] [audio=1] [email=1] [portal=1]` | Open a search hit (`file://`, `audio://`, `email://`, `os://`, teddy) |
| `CALL email.send to=<addr> subj=<s> body=<b> email=1 confirm=1` | Mock send (needs both bits; no gog) |
| `CALL search.query q=<keywords> k=<n> cat=<opt>` | Knowledge search (curated corpus) |

`doc.read` uses the same wire bits as `search.query`: a caller that could not
have found a hit must not open it by guessing the URL. Teddy / portal corpus
bodies need `portal=1`; curated `os://` docs need no personal-data bit.

Host → guest:

| Response | Meaning |
|----------|---------|
| `OK pong` | Alive |
| `OK tools=a,b,c` | Tool list |
| `OK email.search n=<N>` / `ROW id=…\|from=…\|subj=…` / `END` | Mail hits |
| `OK search.query n=<N> backend=…` / `ROW title=…\|…` / `END` | Knowledge hits |
| `ERR <tool> <reason>` | Failure |

Fields use `key=value`; use `|` between fields. Values are single-line; spaces allowed after `=`.

## Capabilities (guest)

Guest `Caps` (setup + live switches) gate COM2 calls. The bridge also checks
wire flags so a forged CALL cannot bypass consent:

| Cap | Wire bit | Tools |
|-----|----------|-------|
| `email.search` | `email=1` | `email.search`, `calendar.list`; revoke → `email.forget` |
| `email.send` | `email=1` + `confirm=1` | `email.send` mock queue; guest Cap + Brief Confirm CTA |
| `search.query` | guest refuse | `search.query` (`email=1`/`files=1`/… opt-in) |
| `workspace.index` | `files=1` | workspace index / file docs; `workspace.forget` |
| `audio.transcribe` | `audio=1` | `audio.transcribe path=…`; Search path picker; `audio.forget` |
| `skills.save` | `skills=1` | `skills.save`; revoke → `skills.forget` |
| `portal.sync` | `portal=1` | `tsearch.sync`, `teddy.*`, `market.*`; `portal.forget` |

Ambient root is forbidden: a missing grant is a hard deny.

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_EMAIL_BACKEND=mock` (default) | Deterministic fake rows (CI) |
| `OS_MCP_EMAIL_BACKEND=gog` | `gog gmail search -j --results-only …` via keyring |
| `OS_MCP_SEARCH_BACKEND=tfidf` (default) | Curated `search/corpus.json` (tSearch-style) |
| `OS_MCP_SEARCH_BACKEND=mock` / `tsearch` | Demo rows / live tsearch-revival |
| `OS_TRANSCRIBE_BACKEND=mock` | Deterministic transcript text (CI; no whisper) |

Guest Search: with Recordings on, Enter on an absolute media path
(`/…/*.wav` and friends) calls `audio.transcribe` then searches the stem.

## Bridge API

Also see skills in [`docs/skills-os-doc-v01.md`](skills-os-doc-v01.md) and search in [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).

## Commands

```sh
make bridge          # build host/bridge
make bridge-run      # listen :7420 (mock)
make run-bridged     # QEMU COM1 stdio + COM2 → bridge (gog if set)
```
