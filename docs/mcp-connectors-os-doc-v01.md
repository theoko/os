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
| `CALL workspace.recent k=<n> files=1` | Top-ranked workspace files for Home |
| `CALL intent.resolve q=<ask> [files=1] [email=1]` | Smart Home planner (act + query + ranked hits) |
| `CALL workspace.forget` | Delete the workspace index |
| `CALL doc.read url=… [files=1] [audio=1] [email=1] [portal=1]` | Open a search hit (`file://`, `audio://`, `email://`, `os://`, teddy) |
| `CALL email.send to=<addr> subj=<s> body=<b> email=1 confirm=1` | Mock send (needs both bits; no gog) |
| `CALL search.query q=<keywords> k=<n> cat=<opt>` | Knowledge search (curated corpus) |
| `CALL update.check [running=<id>]` | Is a newer build published? Reports only |
| `CALL update.download [arch=<x86_64\|arm64>] [wait=1]` | Fetch, verify, and stage that build |
| `CALL update.status` | Download phase and what is staged (local only, never fetches) |
| `CALL update.forget` | Delete staged images and partials |

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
| `workspace.index` | `files=1` | workspace index / `workspace.recent` / file docs; `workspace.forget` |
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

## Software updates

`scripts/publish-os.sh` puts images on `teddysearch.com/tsearch/os/`;
`update.*` brings them back. The bridge downloads, verifies against the
published checksum, and stages into
`~/Library/Application Support/os/updates/`. **Nothing is ever applied.** These
are boot media: the kernel boots read-only and has no filesystem driver, and
even once the Linux substrate makes self-replacement possible, an OS that
rewrites its own boot image from the network is the opposite of a system whose
accesses are explicit. A person flashes the staged image, or points a VM at it.

Two published-site facts the updater is built around, both already paid for:

* **`/tsearch/` has a catch-all.** A missing path answers **HTTP 200 with the
  homepage**, not 404. So every fetch is judged by its *body* — a manifest that
  parses, a checksum list with 64-hex lines, a build stamp with a commit.
  Status codes prove nothing here. `manifest.json` is optional for this reason
  and `SHA256SUMS` is the checksum authority: it is the file publish-os.sh
  uploads and re-verifies both on the server and over HTTPS.
* **A CDN fronts the origin.** The bare `os.iso` URL serves the *previous*
  release for hours after a publish, which is why `os.html` links `?v=<commit>`
  and check-published.sh only warns about the bare one. Downloads use the same
  versioned URL. Without it a perfectly good release arrives as a checksum
  mismatch — which reads to anyone verifying a download as tampering. The
  commit therefore comes from `BUILD-INFO.txt` and a download **refuses** when
  it is unreadable rather than falling back to the stale URL.

`update.check` names a **version** when a manifest is published and a
**commit** otherwise, and says which in `ROW source=`. A commit and a version
are not comparable, so with no manifest it reports `state=undetermined` rather
than guessing "behind".

Not in `LIST`, deliberately: `every_listed_tool_is_dispatchable` calls every
advertised tool with no arguments, and listing these would pull the release off
teddysearch.com on every `make test`.

Not behind a cap either — no personal data is read, nothing outside the staging
directory is written, and nothing is applied. That is a deliberate position,
not an oversight; revisit it if `update.*` ever grows the ability to install.

| Env | Behavior |
|-----|----------|
| `OS_UPDATE_BASE` | Release directory (default `https://teddysearch.com/tsearch/os`) |
| `OS_UPDATE_MANIFEST` | Manifest URL, if not `<base>/manifest.json` |
| `OS_UPDATE_DIR` | Staging directory for downloaded images |

## Bridge API

Also see skills in [`docs/skills-os-doc-v01.md`](skills-os-doc-v01.md) and search in [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).

## Commands

```sh
make bridge          # build host/bridge
make bridge-run      # listen :7420 (mock)
make run-bridged     # QEMU COM1 stdio + COM2 → bridge (gog if set)
make update-check    # is a newer build published?
make update-os       # download it for this machine, verified; applies nothing
make update-os ARCH=x86_64
```
