---
version: 0.9.27
---

# Changelog

## 0.9.27 — 2026-07-25

Home goals get a smart host planner:

- Bridge `intent.resolve` classifies the act (open / search / mail), expands
  synonyms (`paper` → thesis/draft/…), and ranks the workspace index when
  `files=1`.
- Guest `run_goal` prefers that plan + pre-ranked Doc hits, then falls through
  to `search.query`. Offline still uses local keywords. No LLM in the kernel.
- Smoke: paper-style ask ranks the seeded workspace file.

## 0.9.26 — 2026-07-25

Home field is agentic (plan / act / Brief):

- Enter on Home runs `agent::run_goal` under current caps — not bare search.
- Filler words drop (`i wanna work on my paper` → query `paper`); granted
  search / files / mail tools act; Doc rows open in the Reader.
- No LLM in the kernel. Missing grants stay `Need`. Path/media Enter still
  goes to Search / transcribe.

## 0.9.25 — 2026-07-25

Home Recent files opens in the Reader:

- Bridge `workspace.recent` (needs `files=1`) returns top-ranked index rows
  with `file://` URLs for the guest Home strip.
- Guest peeks after Your files is granted; clicking a row opens `doc.read`
  the same way Search / mail already do. Cap refuse never opens COM2.
- Smoke: deny without `files=1`, allow after `workspace.index`.

## 0.9.24 — 2026-07-25

Open calendar events from Brief:

- `calendar.list` ROWs carry stable `id=` (same hash as mail); `doc.read
  cal://{id}` needs `email=1` and returns mock event body.
- Guest arms Event report lines on Brief; click opens the Reader (same path
  as home mail). Cap refuse never opens COM2.
- Smoke: list id → deny/allow `doc.read cal://`.

## 0.9.23 — 2026-07-25

`email.send` mock + explicit confirm:

- New Caps **Send mail** (`Cap::EmailSend`, default off) — separate from Email
  read. Bridge needs `email=1` and `confirm=1`; mock queues only (no gog).
- Inbox Brief arms a draft and shows **Confirm send**; that click CALLs with
  `confirm=1`. Cap refusal never opens COM2. Playbooks stay Info-only.
- Smoke: deny without bits, mock allow with both.

## 0.9.22 — 2026-07-25

Home Recent mail opens in the Reader:

- `email.search` ROWs carry stable `id=` matching the mail graph /
  `doc.read email://…` (mock + gog).
- Guest `MailPeek` stores the id; clicking a home mail row opens the same
  Reader path Search already uses. Cap refuse still needs `email=1`.
- Smoke opens mail via the ROW id; unit tests cover hit geometry + URL build.

## 0.9.21 — 2026-07-25

Calendar is not ambient:

- Bridge `calendar.list` requires `email=1` (same consent as `email.search`);
  mock demo ROW when allowed. Smoke + unit cover deny/allow.
- Guest `fetch_calendar_peek` refuses without `Cap::EmailSearch` (no COM2).
  Morning / inbox / playbook act surface `Event` lines when Email is on.
- Caps Email detail: "Read inbox and calendar".

## 0.9.20 — 2026-07-25

Saved playbooks CALL granted tools:

- After the plan preview, `run_playbook_allowed` issues MCP peeks for tools
  named in the body that are already on (`email.search`, `search.query`,
  portal health). Cap refusal never opens COM2.
- Path/URL/write tools stay Info-only (`audio.transcribe`, `doc.read`,
  `skills.save`, …). Markdown is still not executable; `email.send` stays off.
- Skills chrome copy matches: saved rows run under current grants.

## 0.9.19 — 2026-07-25

Chill 60 fps game loop:

- Main loop always paces at 60 Hz (`anim::FRAME_US_60`) so the UI keeps a
  quiet pulse when the pointer is still — not only when something moves.
- Soft nav hairline breath, ~1.2 Hz caret blink on Home/Search, gentler
  screen entrances (12 frames / 22 px), softer pointer glide, chillier
  startup chime.
- Ambient chrome stays cheap (1 px rule + dirty present); no inference in
  the kernel.

## 0.9.18 — 2026-07-25

The UI adapts to how advanced you are — by choice, not profiling:

- Setup **Experience** step: Guided vs Advanced (explicit; reboot resets).
- Guided keeps plain-language Caps blurbs and longer hints; Advanced shows
  wire tool names and shorter chrome (home/search placeholders, Caps footer,
  setup Bridge/Skills/Done copy).
- Morning brief shortens its plan checklist in Advanced.
- Default grants stay privacy-first at every level. Re-running setup keeps
  the last Experience selection highlighted.

## 0.9.17 — 2026-07-25

Save skills revoke forgets what it wrote:

- Bridge `skills.forget` deletes the user skills tree only (defaults stay);
  idempotent `removed` / `nothing_to_remove`.
- Guest Caps: turning **Save skills** off calls `skills.forget` and refreshes
  the Skills peek so `src=saved` rows disappear.
- Smoke: save → get → forget → list; Caps copy names the revoke consequence.

## 0.9.16 — 2026-07-25

Saved-skill playbook plan preview (still no auto-CALL):

- Unknown / saved skills open a **Playbook plan** Brief; the click path
  fetches the body and names mentioned tools (`email.search`, …) plus
  missing grants. Markdown stays non-executable.
- Starter `skills.save` bodies include suggested tools so the preview is
  useful out of the box; smoke checks `skills.get`.
- Caps / morning brief: Recordings copy points at the Search media path.

## 0.9.15 — 2026-07-25

Recordings path picker on Search + mock transcribe for CI:

- Guest `mcp::transcribe` sends `CALL audio.transcribe … audio=1`; cap denial
  never opens COM2. Search/home Enter on an absolute media path runs it, then
  searches the file stem.
- Grant/setup serial notes that Search is the path picker (no warm without a
  path). Field placeholder mentions `/path.wav`.
- Bridge `OS_TRANSCRIBE_BACKEND=mock` returns deterministic text without
  whisper; smoke exercises the allow path end-to-end.

## 0.9.14 — 2026-07-25

Morning brief files lane + open the built-in corpus:

- `plan_act` notes when Your files is on (Search to open `file://` hits),
  same no-COM2 pattern as Recordings.
- Smoke + unit: `doc.read os://…` returns curated body text with no wire
  bit (`portal` / `files` / … stay off). Completes search→open for docs.

## 0.9.13 — 2026-07-25

Open recordings completes the search→open trilogy:

- Smoke: `doc.read audio://` deny/allow with seeded transcript text before
  `audio.forget`; unit test covers the allow path.
- Teddy corpus bodies need `portal=1` on `doc.read` (`needs_portal_cap`);
  built-in `os://` corpus stays ungated. Reader maps portal / missing
  transcript denials honestly.
- Morning `plan_act` notes when Recordings is on (Search to open audio hits).

## 0.9.12 — 2026-07-25

Open what you find — files and mail, not just locate them:

- Smoke: `workspace.index` / `search.query files=1` / `doc.read` / forget;
  `doc.read email://` needs `email=1` and shows from/subject/snippet.
- Guest `fetch_doc` sends `email=1`; reader denials name the missing grant
  instead of blanket Teddy.
- Caps: Your files detail is "project folders you choose", not the whole machine.

## 0.9.11 — 2026-07-25

Recordings honesty and saved-skill Brief:

- Search tile subtitle includes `audio` when Recordings is granted; empty
  search ladder names `audio.transcribe` after files and mail.
- Every Skills row opens Brief — saved/unknown show playbook body instead of
  a footer blurb.
- Smoke seeds a transcript store, asserts `search.query … audio=1`, then
  `audio.forget` (no live whisper).

## 0.9.10 — 2026-07-25

Email consent matches the other personal-data grants:

- Bridge `email.search` requires `email=1` (`needs_email_cap`); guest peeks
  send the bit and use `max=5` to match the inbox Brief plan.
- `email.forget` deletes the mail knowledge graph; Caps revoke of Email
  purges and refreshes the home peek.
- Home empty-mail copy splits granted+empty vs not granted.
- Smoke asserts LIST / deny / allow / forget for email.

## 0.9.9 — 2026-07-25

Guest finally writes playbooks when Save skills is on:

- `mcp::save_skill` sends `CALL skills.save … skills=1` (one-line `desc=`
  form); cap denial never opens COM2.
- Skills screen **Save starter** CTA + `capability-safe-tools` prove the
  write; list rows surface `src=saved`.
- Skills tile subtitle is `N writable` / `N read-only`; smoke round-trips
  an allowed save and checks `skills.list`.

## 0.9.8 — 2026-07-25

Consent on the wire for skills and markets in CI:

- `skills.save` requires `skills=1` (`needs_skills_cap`); denied saves still
  drain `LINE`…`END` so the peer cannot desync.
- Smoke-bridge lists and cap-checks `market.*` alongside teddy, with a
  best-effort live `market.health portal=1`.
- Docs: mcp connectors and search name the real cap table + market portals.

## 0.9.7 — 2026-07-25

Market portals reach the guest:

- Skill `market-portals` calls `market.health` / `market.fear_greed` under
  `portal.sync`; morning brief peeks both teddy and market health.
- Grant warm includes `market.health`; Search tile says `online` for the
  portal grant; Caps copy names teddy and markets.
- Home stacks Last brief above Recent mail (brief no longer hides the inbox).
- Setup/Skills list seven builtins.

## 0.9.6 — 2026-07-25

Home keeps the agent report:

- After Back from Brief, a **Last brief** strip shows the heading and top
  lines; tap it to reopen. The launcher no longer pretends nothing ran.
- Search tile subtitle follows grants (`docs`, `mail`, `files`, `teddy`)
  instead of the hardcoded "knowledge + email".

## 0.9.5 — 2026-07-25

Revoking Online services forgets what it built:

- Bridge `portal.forget` deletes the teddy corpus cache and portal snapshots,
  and clears in-memory tsearch state (no more OnceLock that outlived revoke).
- Guest Caps toggle calls `portal.forget` on revoke, matching workspace/audio.
- Setup Skills lists all six builtins (teddy-portals was previously hidden).
- Smoke asserts `portal.forget`.

## 0.9.4 — 2026-07-25

Online services means both teddy paths are ready, not merely permitted:

- Setup finish and live Caps grant run `tsearch.sync` then warm `teddy.health`.
- Guest portal allowlist matches the bridge (`teddy.*` + `market.*`); no loose
  `market.` prefix.
- Cap copy names teddy; smoke-bridge asserts LIST + `needs_portal_cap`, and
  best-effort live `teddy.health portal=1`.

## 0.9.3 — 2026-07-25

Teddy is two connectors, not one:

- **Teddy API** — `tsearch.sync` + `search.query portal=1` over
  `teddysearch.com/tsearch/corpus.json` (corpus file is the API).
- **Teddy portals** — live HTTPS tools `teddy.health`, `teddy.fear_greed`,
  `teddy.gex` on the same host. Cap-gated with `portal=1` (markets too).

Guest skill `teddy-portals` runs both paths under `portal.sync` and paints a
Brief. LIST exposes the new tools.

## 0.9.2 — 2026-07-25

Skills act. Clicking a builtin playbook runs a fixed guest plan under the
current grants and opens a Brief: plan steps, then tagged outcomes — or the
name of the switch still blocking the call. No inference in the kernel.

- **Skill runner** (`agent.rs`): `inbox-brief`, `email-triage`,
  `knowledge-search`, `agent-plan-act`, `capability-safe-tools`,
  and (0.9.3) `teddy-portals`.
- After setup, `agent-plan-act` runs immediately so the first screen is a
  report, not an empty launcher.
- Capabilities tile opens the live switches (it only rewrote the footer
  before). Search field no longer restarts setup; a quiet nav **setup**
  control does that on purpose.
- Grants tell the truth: privacy-first defaults, `from_bools` keyed by
  `Cap::ALL`, workspace/portal build-on-grant, teddy scoped to `portal=1`,
  tsearch prewarm restored on the bridge.

## 0.9.1 — 2026-07-25

Setup Skills and the home Skills screen list playbooks from the host bridge
(`CALL skills.list`) over COM2. Offline still shows the ISO builtins. Clicking
a skill row calls `skills.get` for a short body blurb in the footer.

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
