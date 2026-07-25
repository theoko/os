---
version: 0.9.0
---

# Changelog

## 0.9.0 — 2026-07-25

The OS stopped being a landing page and became something you can use: type a
query on arrival and get answers from five sources, behind capabilities you
choose at first boot.

### It does something now

- Keyboard input (PS/2 set 1) and a search screen. Previously clicking a card
  ran a search and wrote the hits to COM1 — invisible unless you were watching
  a serial console — and there was no keyboard driver to type a query with.
- Home is a launcher: a field that takes keystrokes immediately, three tiles
  carrying live counts, recent mail inline. The old hero's primary button only
  restarted the setup wizard.
- Skills and Capabilities screens; capability switches are live, so grants can
  be changed after setup without reinstalling.

### Sources

- **Offline corpus** compiled into the kernel by `build.rs` as a fixed-point
  inverted index — no `ln()` or float division at runtime, because the kernel
  never enables the FPU. Search works with no bridge at all.
- **Your files** — 308 documents indexed from project roots you choose, ranked
  by recency, depth and README-ness, with backup and vendor trees excluded.
- **Email**, folded into a sender→message PageRank graph.
- **teddysearch.com** — 12,448 documents. The site is a client-side app, so the
  corpus file *is* the API; it is fetched, validated and indexed once.
- **Transcripts** — `ffmpeg` + `whisper.cpp`, entirely local, so speech is
  searchable next to everything else.
- Live portal calls to superintelmarkets.com (`market.health`,
  `market.fear_greed`), cached 120s so the guest cannot rate-limit the host.

### Consent

Five capabilities, chosen at first boot, off unless they need to be on:
`email.search`, `search.query`, `skills.save`, `workspace.index`,
`audio.transcribe`. Each is enforced on the wire, not just in the UI.

- The inbox is no longer read before the user consents. It used to be probed at
  boot with default grants — and once the bridge indexed results, that mail was
  written to disk.
- Personal files and recordings each need their own grant; neither rides along
  on `search.query` or on each other.

### Rendering

- Dirty-rectangle presents: **655x** cheaper frames, measured with `rdtsc`
  (67,468k cycles → 103k). Full-screen blits only happen when the screen
  actually changes.
- The cursor is a pre-rendered coverage mask; it used to re-rasterise five
  supersampled polygons — ~185k edge tests — on every mouse move.
- Anti-aliased proportional type from a build-time atlas, vendored Inter.
- UTM: `UpscalingFilter=Linear` (nearest-neighbour was undoing the AA on a
  Retina display), COM2 wired to the bridge (it had one serial port, so the
  bridge was unreachable by construction), and an `intel-hda` device so the
  new PC-speaker chime is audible.

## 0.8.0 — 2026-07-25

- Offline search tier: `build.rs` bakes `search/corpus.json` into a static
  inverted index (sorted vocabulary + postings) with every weight precomputed
  as fixed point — idf and length-normalised tf in Q16, PageRank in Q10 —
  because the kernel never enables the FPU. `mcp.rs` falls back to it when COM2
  doesn't answer, still reporting the bridge as offline rather than pretending.
  Email stays on the bridge: in-kernel Gmail would need TCP/TLS/X.509 in
  `no_std` plus OAuth tokens in the ISO.
- Email knowledge graph: messages from `email.search` fold into a
  `sender -> message` graph, PageRanked, searchable via `search.query`. Stored
  under Application Support beside saved skills — never the repo, since the
  corpus is compiled into the ISO. Bodies are not stored, only sender/subject/
  snippet.
- Capability gate on email content: caps are enforced guest-side, so merging
  mail into `search.query` would have let a guest holding only `search.query`
  read mail the user declined at setup. Email is opt-in per call (`email=1`),
  off by default; the guest asks only when `EmailSearch` was granted. Verified
  over the wire, not just in unit tests.
- Repaint: `fb::Screen` composes into a `.bss` back buffer and blits once.
  Drawing straight into video memory meant ~786k *uncached* MMIO writes per
  full-screen clear plus an MMIO read-modify-write per anti-aliased pixel —
  the flash and the crawl on every click. Cursor motion blits only the two
  cursor footprints.

## 0.7.7 — 2026-07-25

Review hardening: 29 confirmed findings from a multi-agent code review
(adversarially verified), all fixed.

- Bridge: bounded line/body reads (64KB/1MB) so a hostile client can't OOM the
  process; one-line `skills.save … desc=` no longer desyncs the LINE…END
  protocol; `key=value` args may contain spaces per the wire doc; disconnect
  mid-body no longer writes a truncated skill; skill names sanitized in ROW
  output; `skills.get` preserves bodies verbatim (no 120-char cut / `|` swap);
  gog query passed after `--`; tsearch query via env not code injection;
  `snip()` byte/char offset fix.
- Kernel MCP: first-reply timeout raised for the slow gog backend (empty-inbox
  silent failure); 768-byte line buffer fits max legal ROW; UTF-8-safe field
  truncation and valid-prefix decode.
- Serial: `read_line` drains overlong lines through the terminator and drops
  mid-line timeouts instead of replaying tails / partials as complete lines.
- Drivers: cursor save box covers the keyline (no more white trails); PS/2
  9-bit delta signs + overflow bits honored; i8042 drained before command-byte
  read; keyboard bytes rejected once AUX flag proven; UHCI TD/QH stores
  volatile + fenced before frame-list publish, frame-tick wait before scratch
  reuse; PCI `write16` no longer clears RW1C status bits.
- UI/boot: setup footer anchors below content on short framebuffers (clicks no
  longer misroute to rows); framebuffer channel order verified (XRGB) instead
  of assumed; fb-missing boot paths exit QEMU with failure so smoke catches
  them.
- Build/scripts: smoke-bridge waits for the bridge to actually listen; UTM
  quit/reopen polls instead of fixed sleeps; VM name validated + AppleScript
  paths escaped; `make smoke` chmods its script; LIMINE_BRANCH mismatch warns.

## 0.7.6 — 2026-07-25

- UTM: `make utm-bridged` starts the host bridge and adds a COM2 **Serial**
  device in `TcpClient` mode to `127.0.0.1:7420`. Setup's Bridge step re-probes
  on entry (`mcp: bridge live`). Bridge also supports `unix:` listen and
  `OS_MCP_BRIDGE_CONNECT` dial modes for non-UTM use.
- Bridge: scrub CSI/control noise on COM2 so `PING`/`CALL` survive UEFI
  chatter; ignore non-protocol lines instead of `ERR unknown`.

## 0.7.5 — 2026-07-25

- Search: guest `search.query` peek gated by `search.query` cap. Home
  **Connectors** card runs a corpus query and shows the top hit in the footer;
  **Capabilities** / **Skills** cards are clickable too.

## 0.7.4 — 2026-07-25

- Caps: guest-side grant bitset (`caps.rs`). Setup's Capabilities step feeds it;
  `email.search` over COM2 is refused without the grant. Home footer shows the
  active set; Skills CTA reports whether `skills.save` is writable.

## 0.7.3 — 2026-07-25

- UTM: `make utm` refreshes an existing VM bundle instead of delete+create.
  Ghost library rows (name registered, `.utm` missing) are scrubbed from UTM's
  preferences so recreate stops failing with AppleScript -2700.

## 0.7.2 — 2026-07-25

- UI: macOS-style first-boot **setup assistant** (region, bridge, capabilities,
  skills) with click hit-zones and edge-triggered presses.
- UI: home Ready / Skills pills are clickable — Ready re-runs setup; Skills
  updates the footer. Shared CTA hit-test layout with draw.

## 0.7.1 — 2026-07-25

- Mouse: USB tablet interrupt-IN is non-blocking and keeps the TD armed across
  NAKs (no data-toggle flip on empty polls). Fixes "ready but cursor stuck" —
  QEMU abs input moves the pointer (screendump + serial hits); UTM guest logs
  `usb-tablet ready` on the dedicated `piix3-usb-uhci` + `usb-tablet` path.
- UTM: disable UTM's USB input bus; attach one UHCI + one tablet via
  `AdditionalArguments`. Remove orphan `.utm` bundles (no `config.plist`) that
  blocked recreate with AppleScript -2700.

## 0.7.0 — 2026-07-25

- UI: vendored **Inter** (SIL OFL 1.1, `assets/fonts/`) as the default UI font, so
  the build is reproducible off macOS and the ISO is redistributable. System
  fonts remain fallbacks and warn at build time; `OS_UI_FONT` overrides.
- UI: anti-aliased proportional type. `build.rs` rasterizes a real outline font
  into an 8-bit coverage atlas
  at six size/weight cuts; the kernel only blends. Replaces the 8x8 bitmap face
  that made display type blocky and spaced letters on a fixed 8px cell.
- UI: home screen rebuilt against superintelmarkets.com — white page, 44px/600
  hero at -3% tracking, muted sub-copy, accent + tinted pill pair, bordered
  card row. Palette taken from the site (`#1D1D1F` / `#86868B` / `#0071E3`).
- UI: `fill_round_rect` and the new `fill_polygon` anti-alias via 4x4 integer
  supersampling — no floats, since the kernel never enables the FPU.
- Mouse: pointer is now an AA polygon (~12x19) with a white keyline, replacing
  the 44x68 nearest-neighbour bitmap arrow.
- UTM: fix `AdditionalArguments` serialization. It must be a flat list of plain
  strings, one per argv token; a `{"ArgumentString": "flag value"}` entry fails
  to decode and UTM silently drops the VM from its library — which is why the
  pointer stayed dead. Verified `-global usb-tablet.usb_version=1` reaches QEMU,
  putting the tablet on a 12 Mb/s UHCI companion where the guest driver binds it.

## 0.6.3 — 2026-07-25

- Mouse: disable EHCI via PCI (not MMIO) so UHCI companions see the tablet; force `usb_version=1`; HID SET_IDLE/PROTOCOL; don't triple-fault on probe.

## 0.6.2 — 2026-07-25

- USB tablet via UHCI (UTM/SPICE absolute pointer); re-enable USB input bus. PS/2 remains fallback.

## 0.6.1 — 2026-07-25

- UI fix: drop non-ASCII punctuation (bitmap font only has ASCII — em dash / middot rendered as `?`).
- Simpler brand wordmark + faster banded mist background.

## 0.6.0 — 2026-07-25

- Home UI redesign: cool mist atmosphere, geometric `os` wordmark, single CTA; strip hero clutter (skills/mail rows/secondary pills).

## 0.5.3 — 2026-07-25

- UTM: disable USB input bus so `usb-tablet` no longer overrides PS/2 mouse (pointer can move).

## 0.5.2 — 2026-07-25

- UTM: enable `QEMU.PS2Controller` (was false — no mouse packets).
- Larger blue guest pointer (4×) painted before PS/2 init so it’s always visible.

## 0.5.1 — 2026-07-25

- Guest software mouse cursor (arrow) + PS/2 relative input so UTM capture isn’t “mouseless”.
- After framebuffer paint, request QEMU debug-exit then keep an input loop on UTM.

## 0.5.0 — 2026-07-25

- Knowledge search: bridge tool `search.query` (tf-idf × PageRank boost) over `search/corpus.json`, inspired by tsearch-revival’s agent path.
- Optional backends `mock` / `tsearch` (`TSEARCH_DATA`); skill `knowledge-search`; docs `docs/search-os-doc-v01.md`.

## 0.4.0 — 2026-07-25

- Agent skills: defaults in `skills/defaults/`, saved under Application Support via bridge.
- Bridge tools `skills.list` / `skills.get` / `skills.save`; home UI lists builtin skills.

## 0.3.1 — 2026-07-25

- Fix UTM black screen: ISO was never attached (sandbox); bundle `os.iso` into the `.utm` and reload.
- Draw framebuffer UI before MCP probe; shorten COM2 timeouts so UTM isn’t stuck on a blank frame.

## 0.3.0 — 2026-07-25

- MCP-style host connector bridge (`host/bridge`) with `email.search` (mock or `gog`).
- Guest COM2 MCP client; home UI shows email connected + inbox peek.
- `make bridge-run`, `make run-bridged`, `make smoke-bridge`.

## 0.2.0 — 2026-07-25

- Minimal framebuffer home UI inspired by superintelmarkets.com (white, brand-first, blue pill CTA).
- UTM VM defaults to UEFI for a proper GOP display.

## 0.1.2 — 2026-07-25

- Draw hello on Limine framebuffer so UTM’s main window shows the banner (no View→Serial).
- Document `utmctl attach os` for PTTY serial.

## 0.1.1 — 2026-07-25

- Add UTM desktop front-end (`make utm` / `make utm-run`) for Apple Silicon — Parallels cannot run x86_64 guests.
- Docs: `docs/utm-os-doc-v01.md`; Makefile pins rustup paths for BSD `make`.

## 0.1.0 — 2026-07-25

- Initial scaffold: Rust `no_std` kernel, Limine boot, serial hello in QEMU.
- House style: `AGENTS.md`, `VERSION`, architecture + boot docs.
- Host unit smoke for hello message; scripted QEMU serial check.
