---
version: v0.14.0
project: os
updated: 2026-07-31
type: changelog
---

# Changelog

Runtime version is the top-level `VERSION` file (no `v` prefix there).
This file is newest-first. When behavior ships: bump `VERSION`, add an entry
here, update this frontmatter `version:` / `updated:`, then conventional-commit.

## [v0.14.0] — 2026-07-31

### Added — trading-style version control (so we know what we are shipping)

`VERSION` + `CHANGELOG.md` now follow the same discipline as
`~/Desktop/iakovos/trading`: YAML frontmatter carries the current version,
entries are newest-first with `## [vX.Y.Z] — date` headers, and identity cards
require a bump on real work. Linux live images still bake `VERSION` into
`/etc/teddyos-software` and the ISO filename — freestanding kernel ISOs do not
surface it on the “hello” screen (that product line is separate).

### Added — Search intent router (agent decides, product executes)

Search classifies free text into structured intents (`message_reply`,
`project_help`, `freeform_help`, `lookup`, `ambiguous`) via
`linux/teddyos-search/intent.py`, then runs known playbooks. Ambiguous input
offers plain choice rows. Never free-roams; never auto-sends. Design record:
`docs/intent-router-os-doc-v01.md`.

### Added — LinkedIn / WhatsApp “do it” = open + AI draft

`teddyos-linkedin` and `teddyos-whatsapp` open the messaging surface and drive
an AI draft playbook. Reply paths are draft-only — no auto-send. Wired through
Search when the intent is a message reply.

### Added — aspect-aware display lift + auto-update opt-in

`teddyos-display` picks the densest mode that **matches the current aspect**
(blocks ultrawide by default; `TEDDYOS_ALLOW_ULTRAWIDE` escape hatch). Setup
gains an optional software auto-update grant (`software.auto_update`), off by
default. VirtualBox ARM default resolution preference is 1920×1080.

### Fixed — backpacking false positive and project audience

“look at results” no longer scores as backpacking (stopword “at” removed from
lexicon matching). Work-on goals like `tsearch-revival` resolve to CODE-style
project audience more reliably. Session-ready after Connect unblocks “get help”
stuck states; multi-OAuth prefers Claude-first when several are pending.

### Changed — nontechnical UX depth (post-0.13 catch-up)

Multi-persona help, freeform goals, levels, Connect helpers, Ask-all / AI
picker polish, and contract/e2e coverage for the Linux desktop path landed
after 0.13.0 without a version bump — they are part of this tree as of 0.14.0.
Audit: `docs/nontechnical-ux-audit-os-doc-v02.md`.

**Note for operators:** `make virtualbox-arm64` boots the **freestanding
kernel** (“hello” setup). The Linux daily-driver UX above is on
`make linux-iso` / the `teddyos-*-*.iso` live image — not `os-arm64.iso`.

## 0.13.0 — 2026-07-28

### “Work on …” can clone from GitHub

When there is no local folder for a goal (e.g. `i wanna work on tsearch`),
Search checks GitHub if `gh` is signed in or SSH keys work, lists matching
repos, and clones the one you pick into `~/Projects/` before offering AI tools.

### Logs land in one place

Guest keeps a durable trail under `/var/log/teddyos/`: persistent journald
(14 days / 512 MB), rotating per-app files, and daily (plus post-boot) snapshots
via `teddyos-log-collect`. Apps tag the journal as `teddyos-*`. From the Mac,
`./scripts/collect-os-logs.sh` pulls the guest tree plus host serial crumbs into
`logs/os/`.

Snapshots require an explicit setup grant — **Help improve teddyOS**
(`diagnostics.share`), off by default. Without it the collector no-ops;
`--force` is the operator escape hatch only.

### “Work on …” opens an AI tool picker

Typing a goal into Search (for example `i wanna work on iakovos-trading`) no
longer only ranks web hits. The window finds the project folder on the machine
and offers Claude (and Files / Terminal when present). `teddyos-claude` accepts
the project path so Claude starts in that directory.

Each tool row then checks remaining capacity: Claude via `claude auth status`
and `claude usage` (so “Credit balance is too low” or “Not signed in” shows
before you click). Files and Terminal report that they need no credits; Cursor
admits when only “installed” is knowable.

The catalog is every AI tool we know how to launch (Claude, Cursor, Windsurf,
Codex, Gemini, Aider, Amp, Crush, Goose, Ollama, VS Code, VSCodium, Zed) —
only installed binaries appear. Choosing a tool records it for 30 days under
**Recently used**, sorted above the rest with a badge on the row.



### Live ISO records the host commit

`linux/iso/build-iso.sh` runs on a guest that receives the tree without
`.git`, so `/etc/teddyos-software` used to say `commit=unknown` and every
fresh install offered an update. `scripts/build-linux-iso.sh` (and
`make linux-iso`) resolves the commit on the Mac, passes `TEDDYOS_COMMIT`,
and pulls the ISO into `dist/`.

### Standalone cleanup — no more dead bridge surface

The host bridge is gone; this pass removes what still pretended it was not.

- Identity cards (`AGENTS.md`, `CLAUDE.md`) match standalone only: no COM2
  connector, no `bridge-run`, no `host/bridge/`.
- Deleted dead stubs: `refresh-index.sh`, `make-portal.sh`, `substrate-proof.sh`,
  the refresh LaunchAgent, and the MCP connectors design doc.
- UTM and VirtualBox launchers no longer wire or start a host bridge.
- Skill playbooks and the health workflow describe offline reality; email and
  live portals stay catalogued as offline until a guest network path exists.
- Tracked `__pycache__` bytecode removed (already gitignored).

### amd64 boots, and the BIOS menu is teddyOS

The first amd64 image. Verified by booting it, not by reading the build tree:
isolinux -> kernel -> GNOME -> the setup journey, on SeaBIOS, which is the
firmware VirtualBox on x86 defaults to and therefore the path most people who
download this will actually take.

Getting there needed two fixes the build's own assertion caught before either
could ship.

- syslinux writes its keyboard accelerator as a caret INSIDE the label —
  `Start ^installer`, `with ^speech synthesis`. Patterns written against the
  plain words matched some labels and not others, which is worse than matching
  none: the speech entry failed while the generic one succeeded, leaving
  "Install teddyOS with ^speech synthesis" on the menu. And `Advanced install
  options` and `Utilities` had no BIOS rename at all.
- The staleness assertion grepped the whole isolinux directory, and syslinux
  ships `libgpl.c32` — a binary containing the string "Utilities". It could
  never have passed on amd64 no matter what the menus said. It reads
  `*.cfg` now.

### Nothing in the desktop says Debian any more

- The installer opened as "Welcome to the Calamares installer for Debian 13"
  with a Debian swirl, at the moment somebody commits their disk. It carries
  teddyOS branding, and a slideshow — which is not optional: a branding
  component declaring `slideshowAPI` without a `slideshow` is rejected whole,
  and Calamares then exits before drawing anything. That presented as the
  Install button doing nothing at all.
- The login banner read "Debian GNU/Linux 13 teddyos" on every text console and
  serial session. Found on the amd64 serial console, which is the one place
  nobody thought to look.
- The Web tile wears Chromium's own icon, copied at all seven sizes into our
  own icon name so the theme cannot substitute Chrome's four-colour mark —
  which would put Google's trademark exactly where Safari's compass was.

### Errors read as sentences

`<urlopen error [Errno -3] Temporary failure in name resolution>` was being
rendered in the Search window. Accurate, useless, and alarming: it looks like
damage when the news is "you are not online". Network failures are translated
at the point they are produced, so the window and the terminal agree.

- Names under the dock icons. Six tiles and no words is a guessing game for
  the people this is for, and Dash to Dock's labels only appear on hover —
  something you do only if you already suspect a tooltip exists.


### teddyOS runs on VirtualBox

The live image booted to a black screen on VirtualBox's Apple Silicon build and
stayed there. Five images shipped before the cause was found, because every
symptom pointed one layer too low: a frozen boot menu, then a hypervisor
assertion, then a missing DRM device. All three were real; none was it.

The cause was a missing package. GNOME's session is Wayland, which needs a
DRM/KMS device. VirtualBox's ARM machine exposes its framebuffer only through
EFI GOP, so the kernel registers `efi-framebuffer.0`, `simpledrm` has nothing
to bind to and `/dev/dri` never appears. GDM turns Wayland off by itself,
looks for an X session, finds `/usr/share/xsessions` empty, and dies with
`no session desktop files installed` — restarting forever behind a black
screen while the text console works perfectly.

`gnome-session-xsession` is only a *Recommends* of `gnome-core`, so
`--apt-recommends false` had silently removed it. The same flag removed
`user-setup` once before.

- `gnome-session-xsession`, `xserver-xorg-core`, `xserver-xorg-video-fbdev`
  and `xserver-xorg-input-libinput` ship now. fbdev draws on `/dev/fb0` and
  needs no GPU at all, and GNOME Shell keeps its X11 backend in 48, so this is
  the real desktop — dock, theme and extensions — not a reduced one.
- The build asserts a Wayland session, an X session and the fbdev driver are
  all present. Losing any of them now fails the build instead of shipping a
  black screen.
- A machine with genuinely no graphics device explains itself on the console
  instead of showing nothing.

### Search no longer needs a terminal

Making the search box return anything took four commands and two error
messages: `teddyos-search`, `teddyos-setup`, search again, then
`teddyos-search --sync`. Granting "Search the web" was permission, not an
index — nothing ever downloaded the 68 MB corpus, so the machine could only
say so in a note under an empty result list.

- Setup downloads the index itself, on its last screen, with a progress bar.
  Detached, so clicking through does not cancel it.
- Every command name is gone from the GUI. The window said
  `run teddyos-search --sync` to people who have never opened a terminal.
- The Search window renders `notes` and `errors`. It computed the reason a
  search came back empty and discarded it, so "nothing found" and "the index
  is still downloading" looked identical.

### Search stopped eating the machine

Moving the dock's Search through the capability sandbox — which it had never
entered, making the setup screen's central promise false — turned a
once-per-session 67 MB corpus parse into a per-keystroke one. The cache is a
module global and cannot outlive a process, so every query allocated 475 MB
and threw it away. On a live system whose cache directory is in RAM, that
OOM-killed the desktop.

- `teddyos-search --serve` enters the sandbox once, parses once and answers
  queries over a pipe. 475 MB once; requeries in 0.08 s.
- It still refuses rather than falling back to an unconfined search.

### The desktop stops advertising Debian

- The installer said "Welcome to the Calamares installer for Debian 13" with a
  Debian swirl — at the moment someone commits their disk to an OS they have
  never heard of. It carries teddyOS branding now.
- **Install teddyOS** is in the dock. The live image had no way to keep
  itself short of rebooting into a different menu entry.
- The bookmark bar was Debian.org, Latest News and Help. It is now six links
  named for what they are for.
- The browser icon was **Safari's compass on Chromium** — misleading, and
  someone else's trademark. Search, Web, Claude and Install are drawn.
- On amd64 the BIOS boot menu was stock Debian with `timeout 0`, which in
  syslinux means wait forever. Both menus are branded and timed now.

### Claude is an application

It was installed, symlinked to `/usr/local/bin/claude`, and reachable only by
opening a terminal and typing a name you had to already know — on a desktop
whose stated audience does not know what a terminal is.

- A window: a terminal with one job, no shell prompt, no tabs, nothing left
  running when it closes.
- `teddyos-whatsapp.desktop` was listed as a dock favourite and never created
  by anything, so GNOME silently dropped it and the dock showed three icons
  where four were intended.
- Apps set `prgname` to their application id. GNOME matches a window to its
  launcher by WM_CLASS, which under X11 comes from the program name — so on
  exactly the machines the X11 fix rescued, a running app appeared twice.

### Builds say which build they are

- Images are named `teddyos-VERSION-ARCH-BUILDID.iso` and the id is on the
  boot splash and in `/etc/teddyos-build`. Three fixes were reported as "still
  broken" while an older image was being booted.
- Both live entries carry a serial console, `tty0` named last so `/dev/console`
  stays the screen. A VM that boots to a black rectangle can now be handed a
  log instead of guessed at.

## 0.13.0 — 2026-07-27

### The mouse works again after setup

Every click outside the setup wizard had stopped registering. `prev_buttons`
— the previous frame's button state, which is what a press is measured
against — was being advanced at the top of the input loop, before any handler
ran, so `was_down` always equalled `left_down` and no press ever read as a new
press. Back, the home tiles, Brief `Doc` rows, Search results and the
capability switches all went dead together, while the pointer still moved and
still painted its hover rails, so the machine looked alive and answered
nothing. Setup kept working because it reads the button state rather than its
edge — which is exactly why the failure began the moment setup ended.

The assignment at the bottom of the loop, which is the correct one, was always
there. Removing the early one restores every click.

- `make e2e` — the only suite here that clicks — has been unrunnable since the
  standalone conversion: it defaulted to starting a bridge through a script
  that commit deleted, and died in argument handling before booting anything.
  It defaults to offline now, the way every shipped image runs.

### Documents found offline can be opened

The corpus already carried a 320-character extract per document; `build.rs`
read it for tokens and threw it away. So the machine could find a document and
never show one, and every row on a device that is never anything but offline
dead-ended at "cannot open documents".

- The extract is baked into the kernel alongside the title and URL, and
  `mcp::fetch_doc` reads it when there is no bridge — one place, so a Search
  row, a Brief `Doc` row and anything added later all open the same document.
  The reader wraps it at word boundaries and ends with a line saying it is an
  extract, so a stored opening is never mistaken for a whole document.
  The ISO grows 70 KB.
- A row is drawn openable exactly when the image stores its text. Offline rows
  used to carry a URL whatever was behind it, which is what made every one of
  them invite a tap and then refuse. `search.rs::body_for` is the single
  answer to "does this open?", and the Search screen, the offline query and
  `Brief::push_result` all ask it.
- The knowledge and playbook lanes push through `Brief::push_result` like
  every other lane, instead of tagging their rows `Hit` and discarding the
  URL — the same query used to answer openable rows on Search and dead titles
  on the Brief.
- Copy follows: "Titles only - no text stored for these." replaces "the local
  index cannot open documents", and the note under the search field no longer
  says "Bridge offline" on a machine with no bridge to start.

### A finished answer stops looking like a stuck one

Asking the machine for "nvda" left a screen titled **Working on it** above four
cards, the last of which said nothing was found. Nothing draws a Brief until the
run has returned, so that title was false every time anyone read it — the words
in the largest type on the screen said the machine was still thinking.

- The Brief title now reports the outcome: `2 to open`, `3 found`,
  `Nothing found`, `Needs a grant`, `Draft ready`. Openable rows lead the count,
  because tapping one is the next thing to do; a missing grant outranks any
  count, because it is the thing to fix.
- A one-word goal no longer prints itself twice. `Goal: nvda` and `Query: nvda`
  are one fact on two cards; the Query card now appears only when the machine
  searched for something other than what was typed.
- An empty answer says what to try, not only what failed. "Nothing on this
  device matches that." is followed by a `Next` row, and the old "Bridge
  offline - local keywords only." caption is gone from standalone builds — it
  printed on every run this build can do, so it was a caption, not news,
  costing a row on a screen whose whole answer was four cards.

Four cards became three, and the card that ends the screen now points somewhere.

### The machine can find your own work

Every question about the owner's own files came back "No offline hits for that
query." Search was not broken — the shelf was empty. The kernel could only
answer from `search/corpus.json`, and that held sixteen documents, all about
the OS itself.

- `scripts/bake-corpus.py` merges the workspace index into the baked corpus:
  275 documents now, 259 of them the owner's. `search/seed.json` keeps the
  curated OS documents that were there before.
- Non-ASCII is folded on the way in, Greek transliterated rather than dropped.
  The font atlas covers 0x20..=0x7E and the tokenizer emits ASCII runs only, so
  140 of the 320 titles would otherwise have drawn as holes and indexed as
  nothing — baked in and unreachable. `build.rs` now fails the build on a
  character the atlas cannot draw instead of shipping it.
- Generated files are left out: `.egg-info`, lockfiles, `requirements.txt`, and
  anything under twelve tokens. The scorer divides term frequency by document
  length, so a four-word `top_level.txt` outranks a real document that discusses
  the term at length. 54 such entries were dropped.
- Result-row URL slots hold 128 bytes, up from 72, and `search.rs` const-asserts
  that every baked title and URL fits. Truncation here is not cosmetic: 38 of
  the owner's paths were over the old slot, `copy_into` cuts in silence, and two
  paths agreeing for 72 bytes collapse into one row — a document silently erased
  from every answer it belongs in.
- `make publish-os` refuses to upload an image built from a personal corpus. The
  index is compiled into the ISO, so publishing one baked from the workspace
  would put the owner's titles and 320-character snippets on a public download.

### The status dot stops reporting a fault that cannot happen

Since the standalone conversion `ping_bridge` answers Offline for every caller
forever — there is no bridge to answer otherwise. The nav dot went red on first
boot and stayed red on a machine with nothing wrong with it, which reads as
"your search is disconnected" and sends people looking for a host this image
deliberately does not ship.

- A standalone build draws the word `status` where the dot was. It opens the
  same screen and claims no fault. Hit-testing follows what was drawn.
- `make test-host` now runs the suite twice, the second time with
  `--features standalone`. Every shipped ISO is built that way, and until now no
  test ever compiled those branches — which is how a dot that could only be red
  reached first boot.

### Two ranking tests measured the corpus, not the scorer

Both asserted a specific document title. Re-baking the index broke them, and one
was not testing what it claimed: term frequency is divided by document length,
so a four-word file beats a long one on any shared term whatever the weights are.

- `rare_terms_outweigh_common_ones` checks the score `query` delivers against the
  whole formula, on a term in exactly one document — the case where the answer is
  knowable.
- `pagerank_outranks_a_stronger_tf_idf_match` watches a real ranking for a
  document delivered above one with strictly higher term frequency, which nothing
  but the blend can do. It recomputed the blend before, which only proved the test
  could multiply — it passed with the blend deleted.

## 0.12.0 — 2026-07-27

### A machine can fetch the update it was told about

`update.check` could say a newer build existed. Nothing could go and get it, so
the answer ended at "there is one" — and against the live site it did not even
manage that: the URL it read answered with the homepage.

- `CALL update.download [arch=…] [wait=1]` downloads the published image for an
  architecture, verifies it against the published checksum, and stages it in
  `~/Library/Application Support/os/updates/`. `update.status` reports progress
  without touching the network; `update.forget` deletes staged images *and*
  partials, because "off means gone" applies to megabytes fetched on somebody's
  behalf.
- Downloads use `?v=<commit>`, the URL `os.html` links. A CDN fronts the origin
  and the bare URL serves the previous release for hours; fetched that way, a
  good release arrives as a checksum mismatch, which reads to anyone verifying
  a download as tampering. The commit comes from `BUILD-INFO.txt`, and a
  download refuses when it is unreadable rather than falling back to the stale
  URL.
- Verification happens before staging, never after. A mismatched download is
  deleted rather than left at the final path — an unbootable ISO on a USB stick
  explains nothing about why.
- Nothing is applied. These are boot media, and what a machine boots from is a
  thing a person changes on purpose, not a side effect of a status check.
- Runs in the background by default and reports through `update.status`. The
  guest declares the bridge offline after about ten seconds, which is well
  short of a 16 MB download; `wait=1` is for shell callers, which have no such
  timeout.

### The updater stops reading a web page as a release

`/tsearch/` serves a catch-all: a missing path answers **HTTP 200 with the
homepage**. `manifest.json` was never published, so every installed machine
read 88 KB of HTML and reported `manifest_is_not_json` forever.

- `SHA256SUMS` is now the checksum authority — the file `publish-os.sh`
  actually uploads and re-verifies on the server and over HTTPS. A manifest is
  optional and adds the version.
- Every parser judges the body, never the status code: checksum lines must be
  64 hex digits, a commit must look like one (`commit: unknown` is refused —
  "unknown" in a URL fetches the stale cached image).
- `update.check` names a version when a manifest is published and a commit
  otherwise, and says which in `ROW source=`. With only a commit it reports
  `state=undetermined` instead of comparing a commit to a version and calling
  the result "behind".
- `publish-os.sh` now builds and uploads `manifest.json`, and verifies it by
  *body* — a 200 here would prove it published and prove it missing equally
  well. `check-published.sh` watches for it and warns, since a visitor's
  download works without one but no machine can tell whether it is behind.

### Entry points

- `make update-check` / `make update-os [ARCH=…]`, and `scripts/update-os.sh`.
  It refuses to run against a bridge that predates the feature rather than
  reporting the confusing failure that causes — a shared long-lived bridge
  serving an older build has cost this repo an afternoon before — and does not
  kill it, since QEMU/UTM sessions may be mid-boot on the same port.
- `make-usb.sh` accepts `ISO=/path/to/image`, so a staged download can be
  written without first being copied over the build output. That copy step is
  where the wrong image gets flashed.

## 0.11.0 — 2026-07-25

### Native ARM64 VirtualBox desktop

- Added an ARM64 low-device page-table window for VirtualBox's PCI ECAM and
  MMIO BARs, while keeping RAM and framebuffer access in Limine's higher-half
  map.
- Added a polled OHCI host driver with USB enumeration and HID keyboard,
  relative mouse, and absolute tablet report handling. VirtualBox's optional
  `SET_PROTOCOL` stall is tolerated when the descriptor-defined report format
  is already usable.
- Fixed VirtualBox's eight-byte absolute-tablet report layout and completed
  the OHCI done-queue/WDH handshake, so pointer reports are delivered
  continuously instead of the controller stopping after an unpublished TD.
- ARM device MMIO is mapped before the first PL011 probe, and OHCI accesses
  use plain non-writeback AArch64 loads/stores to avoid VirtualBox pinning an
  optimized guest on valid pre/post-indexed MMIO instructions.
- ARM animation and wait timing use the architectural counter. Input polling
  runs at a bounded 1 ms cadence; rendering continues to use cached
  composition and dirty-rectangle presentation.
- ARM64 images now use the optimized release kernel by default. Cursor moves
  restore their saved background on every architecture, eliminating the
  initial center ghost and pointer trails.
- First-run controls now respond before activation: Continue gains a calm
  intent halo, selectable rows tint on hover, and Back underlines. A seven-dot
  journey rail makes progress through setup visible without adding more copy.
- Pointer reports are coalesced into one 60 Hz visual path. Precise movement
  settles over a few frames, fast flicks land on the next frame, and clicks
  still snap exactly to their hit target.
- VirtualBox's post-firmware PL011 output is muted so a full debug FIFO can
  never stall PCI/input initialization.
- `make virtualbox-arm64` now builds the ARM ISO, creates or refreshes the VM
  with QemuRamFB plus OHCI USB keyboard/tablet, reattaches the rebuilt ISO, and
  launches it. Its VM-creation command uses VirtualBox's supported `--ostype`
  spelling.
- Verified in the real VirtualBox VM: first-boot setup advances via the
  emulated USB keyboard, the home search field receives `nvda`, and injected
  absolute-tablet positions move one clean cursor across the desktop. The
  live welcome screen also shows hover feedback and advances once into the
  visible setup journey.

### The agent only promises what it can open

- A Brief row is tagged by what it can deliver, decided once at the sink:
  `Doc` rows open when tapped, `Hit` rows are findings the guest cannot open.
  Lanes could previously draw a `Doc` they never armed, which is how the
  offline guest listed three `Doc` rows above "No openable hits - refine the
  ask." Arming now also requires the push to have appended, so a title
  deduplicated against an earlier row can no longer record its URL against
  the next line.
- The offline search ranks the question that was asked. It had discarded the
  query for a hardcoded showcase phrase, so a brief on `nvda` answered with
  corpus documents about capabilities while claiming "local keywords only".
- A URL too long for its 72-byte slot is no longer stored cut. A truncated
  URL still reads as present, so the row was drawn openable and the tap
  resolved to a path the bridge cannot find; deep workspace roots reach this
  routinely, and two documents sharing a 72-byte prefix also collapsed into
  one. Rows that cannot be stored whole are `Hit`s.
- An empty offline answer is explained once. The guest had stacked three
  sentences — bridge offline, no hits, refine the ask — under a single empty
  result. Offline findings now close with "Titles only - reading needs the
  bridge." because no rewording of the goal can open a baked-index title;
  "refine the ask" is kept for when the bridge is up and the advice works.
- The duplicate-row assertion compares findings, not metadata. A one-word goal
  draws "Goal: opportunistic" above "Query: opportunistic" — two facts about
  one word, which the kernel deliberately allows and the harness read as one
  document listed twice. Its can-it-fail proof was seeding that same shape, so
  correcting the assertion alone would have left it unable to fail; the proof
  now seeds one document tagged both Doc and Hit, the defect it describes.

### A standalone image stops waiting for a host that will never come

- `make standalone-iso` (and `standalone-arm64-iso`) build a bare-metal image
  under the new `standalone` kernel feature. The guest skips the COM2 probe
  outright instead of spending `TIMEOUT_PING` before every bridge-touching
  action to learn what the image already knows. Capabilities are unchanged:
  `BridgeStatus::Offline` was always a modeled state, so this changes when the
  guest asks, not what it can do. The ISO is named separately because the two
  images are not interchangeable.
- Offline copy no longer names a bridge on machines that cannot have one.
  "Bridge offline for mail." describes something the user can fix by starting
  the host; on standalone hardware it points at a remedy that does not exist,
  so that image says "Mail is unavailable on this device." instead. The worst
  offenders were the ones naming a command: the status note read "Bridge
  offline - run: make utm-bridged", and first-run setup had a whole "Connect
  the Bridge" step telling the reader to run `make bridge-run`. All 21 lines
  now live in `kernel/src/copy.rs` and switch together.
- The wording is pinned from both sides — the standalone build must name
  neither "bridge" nor a `make` command, and the hosted build must keep saying
  "bridge" — so the copy cannot pass by going vague in both. Search-source
  lines deliberately keep the word "offline", which stays true on a machine
  with no network and promises no remedy.
- The arm64 e2e harness learned the standalone spelling of the setup step.
  Renaming the screen made it fall through to the catch-all "any screen with a
  Back button" signature, which sent the wrong key and looped the journey for
  12 steps instead of reporting an unknown screen.

### A busy port names the process holding it

- A bridge that cannot bind its TCP port now reports which process owns it and
  exits, instead of panicking. Under the `com.os.mcp-bridge` LaunchAgent the
  bare panic was near-unreadable: `KeepAlive` restarted the bridge every
  `ThrottleInterval` and the log filled with identical aborts, saying nothing
  about the stale `make bridge-run` actually sitting on 7420.

### The morning brief explains itself

- Corpus rows say where they came from. The brief that opens after setup listed
  three bare titles — "Agent skills", "Architecture capability IPC", "os
  identity" — directly under "Bridge offline for mail" and "Bridge offline for
  calendar", with nothing on screen connecting them to anything. Read in order
  it said the bridge was unreachable and then produced documents from nowhere.
  One row of provenance now precedes them, and says why they do not open when
  the bridge is down.
- Corpus rows open when there is something to open. The lane pushed the
  unopenable `Hit` tag unconditionally, so a document could never be tapped even
  with the bridge up and a URL in hand — against the playbook's own step 5,
  "arm Doc / Event rows the user can open". Offline the rows still read `Hit`,
  correctly: `search_offline` carries titles and no URLs.
- The plan lists only the steps this lane performs. It opened with "Restate:
  what matters right now" and then never restated anything, because the morning
  brief and the skills list both reach it with nothing typed. A four-step
  checklist that delivers three reads as a step that silently failed.
- Dropped the `Plan`-tagged row from the report. It put the word Plan on screen
  as both the checklist heading and a row tag meaning something else, and its
  text only restated the two plan steps above it. `Brief::lines` holds eight
  rows and this lane can fill all eight, so that slot is what the provenance
  line spends — adding a row instead would have silently evicted "Recordings
  on" off the bottom of the screen being fixed.

### Setup owns the screen until it is finished

- Boot no longer paints Home before setup has run. Three `draw_home` calls sat
  on the way to the main loop, and Home is a lie before consent: it offers a
  query box and capability cards under an empty grant set. On x86-64 the window
  was milliseconds; on ARM64 the USB probe sits inside it, so the guest showed a
  complete Home screen for over a second and then replaced it with Welcome.
  That is what made the ARM64 end-to-end run report that Enter on Home went back
  to the welcome screen — it never left setup, and the harness had photographed
  and typed into the pre-setup Home paint.
- The "which input is missing" note is drawn on the Welcome screen. It was
  painted onto Home during boot, where setup overdrew it moments later, so on a
  machine with no driveable keyboard or pointer the one sentence explaining why
  nothing responds was never actually readable. Welcome is the screen such a
  machine is stuck on, and a deliberate setup restart keeps the note.
- `scripts/e2e/arm64.py` asserts that the screen a fresh boot lands on is the
  screen it stays on. Nothing has been typed at that point, so a screen that
  changes by itself is the guest overpainting, and the run now says so instead
  of blaming the input path for the keystrokes it aimed at a dead frame.

## 0.10.0 — 2026-07-25

Crashes now report themselves, the search box understands sentences, and there
is a supported path onto real x86-64 hardware.

### Crashes say where they happened

The kernel had no interrupt descriptor table. Any CPU exception escalated
fault → double fault → triple fault, and the machine reset with nothing written
anywhere — "it just randomly crashes", with no way to find out what. `fault.rs`
installs handlers for all 32 CPU vectors that print vector, error code, `rip`
and (for page faults) the faulting address to COM1, then halt where they are.
Verified in a real boot:

    os: FAULT page fault vec=0xe err=0x2 rip=0xffffffff80005b79 addr=0xdeadbeef

The panic handler also exited silently — fine under QEMU's debug-exit device,
invisible under UTM, which has none. It now prints file and line first. Stub
error-code shapes are checked against the PCI spec table at compile time; a
mismatch there would shift the frame and print a plausible, wrong `rip`.

### The search box reads sentences

`agent.act` replaces `search.query` as what the guest calls. It classifies the
goal (resume / find / read / catch-up / ask / keywords), strips filler, gathers
only from granted sources, and returns a sentence plus rows that open. Typing
"i wanna work on my paper" no longer throws away every word that carried the
meaning. When the goal really is keywords it falls through to the same scoped
index as before, so it is never worse than what it replaced.

Two confidently-wrong answers found by running it and fixed: "my paper"
returned `CHANGELOG.md` and `AGENTS.md` (repo furniture is now excluded from
the "documents people write" guess), and "what did i miss" returned six-week-old
notes because `miss` matched "missing" (question words are filler now, and
catch-up ignores search terms entirely and asks about recency).

The sentence is also no longer allowed to lie: it said "5 matches" above three
visible rows, because the bridge counted what it found and the guest could only
hold three. The row limit now flows into the agent before the sentence is
written, and both sides share `search::MAX_HITS`.

### Real hardware

`make usb` writes the ISO to a USB stick. Every guard is load-bearing since it
writes a raw block device: refuses internal disks, refuses partitions, refuses
the running system volume, requires the device to be named and then retyped,
and re-reads the stick to compare SHA-256 rather than trusting `dd`.

This exists because nothing on an Apple Silicon Mac can virtualise x86 —
VirtualBox and Parallels both refuse outright, and UTM only works by emulating
every instruction, which is also why it cannot reach 60fps.

Booting real hardware exposes an honest gap, so the OS now reports it instead
of hiding it: it drives PS/2 and a UHCI USB tablet, and a modern laptop has
xHCI and often no i8042 at all. It would boot to a perfect home screen with a
cursor that never moves. `pci::usb_survey` counts the controllers present and
the status line says which half is missing. See `docs/install-os-doc-v01.md`.

### Fixes

- `make utm-bridged` aborted with `Connection refused`: the bridge built its
  12k-document index *before* binding, and `ensure-bridge.sh` reported success
  after a fixed 0.5s sleep. Prewarm moved to a thread after bind; the script
  now waits for a real accept. Returns in 0.49s with the port live.
- Setup's "Go Back" was drawn off-screen on Capabilities — the footer had no
  clamp, so six rows pushed it past the bottom edge and the only way out of the
  step was forward. Clamping alone made it overlap the last row instead, so
  rows now size to the space actually available. Tested at 800x600, 1024x768
  and 1280x800 that no zone leaves the screen and no two zones overlap.
- Recent-mail rows on the home screen open. They carry an `email://` id and
  `doc.read` resolves it behind the email grant.
- An offline search said "the bridge is offline" twice in two wordings.
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
