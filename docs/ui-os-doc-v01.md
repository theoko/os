---
project: os
type: ui
purpose: framebuffer home screen (ink-and-signal)
status: active
---

# UI — home screen (v02)

## Direction

**Ink & signal** — cool mist atmosphere, geometric `os` wordmark, one headline,
one supporting line, one CTA. First viewport is a composition, not a dashboard.

## Palette

| Token | Hex | Role |
|-------|-----|------|
| BG_TOP / MID / BOT | `#E8EEF4` → `#FFFFFF` | Vertical mist |
| INK | `#0E1621` | Primary type / wordmark |
| INK_MUTED | `#5A6674` | Supporting line |
| SIGNAL | `#006ACC` | CTA + accent rule |
| ONLINE / OFFLINE | green / red | Tiny bridge dot only (standalone builds: always OFFLINE) |

## Composition (first viewport)

1. Hairline top + quiet `capability os` / bridge status (not the brand)
2. Geometric **os** wordmark (ring + carved S) — brand-first
3. One headline, one muted sentence
4. One **Ready** pill
5. Short signal hairline; version footer whisper

No skill lists, mail rows, or secondary pills in the hero.

**Constraint:** the 8x8 framebuffer font is ASCII only (`0x20..=0x7E`).
Never put em dashes, middots, or curly quotes in on-screen strings — they become `?`.

## Experience level (v0.9.18)

Setup asks **How should it feel?** — Guided or Advanced. Chosen explicitly;
not inferred from caps, bridge, or click history. RAM-only until reboot.

| | Guided | Advanced |
|--|--------|----------|
| Caps blurbs | Plain language (`detail`) | Wire names (`email.search`, …) |
| Home / Search placeholders | Longer teaching copy | Short query / path hints |
| Morning brief | Full plan checklist | Heading + at most two plan lines |
| Default grants | Privacy-first (docs only) | Same — Advanced is not ambient root |

Code: [`kernel/src/level.rs`](../kernel/src/level.rs), setup Experience step.

## Home agent (v0.9.26 / v0.9.27)

**Standalone note (since the 2026-07-27 bridge removal):** the smart half of
this feature is gone, not the feature. `agent::run_goal` still runs on every
build. What changed is where the plan comes from: `intent.resolve` was a host
bridge tool, and `kernel/src/mcp.rs` now returns `BridgeStatus::Offline`
unconditionally in a standalone build (`fetch_intent_plan`), so the guest
never gets host-side synonym expansion, act classification, or ranked
`file://` hits. `run_goal_with_plan` (`kernel/src/agent.rs`) falls back to a
local path that was always there for the "bridge not started yet" case and is
now the only path: keywords pulled straight from the goal text
(`keywords_from_goal`), a fixed 4-line generic plan (restate → pick tools from
grants → search under those grants → report openable hits), and an inline
"local search only" notice. It still CALLs `search.query` under the
`SearchQuery` grant, which still answers — see Recent files/search below.
Example: `i wanna work on my paper` → generic plan → keyword query `paper` →
local corpus hits → Reader. No model in the kernel — missing caps stay Need,
same as before.

## Recent mail, recent files, calendar events — removed (standalone)

All three of these (v0.9.22, v0.9.25, v0.9.24) were host-bridge features with
no standalone equivalent, and are gone in a standalone build:

- **Recent mail** listed inbox rows from bridge `email.search`, opened via
  `email://{id}` in the Reader.
- **Recent files** listed top-ranked workspace files from bridge
  `workspace.recent` (after `workspace.index`), opened via `file://…`.
- **Calendar events** listed upcoming events from bridge `calendar.list`,
  opened via `cal://{id}`.

All three needed a live host process reading the user's real mailbox,
filesystem, or calendar and handing rows back over COM2. `host/bridge/` was
deleted by the standalone refactor, and the guest query paths for all three
now return `BridgeStatus::Offline` unconditionally
(`kernel/src/mcp.rs`: `fetch_mail_peek`, `fetch_files_peek`,
`fetch_calendar_peek`) — Home and Briefs simply render nothing for these
rows rather than erroring. There is no local-only version of "read my actual
inbox/files/calendar" the way there is for search: those three depend on
host state a standalone kernel has no way to reach, not on data that could be
baked in at compile time. If bridge-mode features come back, restore
`host/bridge/src/{workspace,tsearch}.rs` (recovered at
`git show f2ccb72~1:host/bridge/src/workspace.rs`, etc.) alongside the calls
into these three functions.

## Search (works standalone)

`search.query` is the one connector that never needed the bridge to begin
with (`kernel/src/search.rs`): when the bridge isn't there, the guest answers
from a static index built from `search/corpus.json` at compile time. This is
what still backs the Home agent's Doc rows above — it needs no network, no
credentials, no host state, so it is unaffected by the standalone refactor.

## Chill 60 Hz loop (v0.9.19)

After the framebuffer is up, the guest runs a paced **60 fps** game loop
(`anim::FRAME_US_60`). Ambient motion — nav-rule breath, caret blink,
soft pointer glide — keeps going when input is idle. Screen entrances are
short eased slides; the startup chime is quieter and a touch slower.

Code: [`kernel/src/anim.rs`](../kernel/src/anim.rs), [`kernel/src/main.rs`](../kernel/src/main.rs).

## Code

- [`kernel/src/ui.rs`](../kernel/src/ui.rs)
- [`kernel/src/fb.rs`](../kernel/src/fb.rs)
- [`kernel/src/level.rs`](../kernel/src/level.rs)
- [`kernel/src/anim.rs`](../kernel/src/anim.rs)
