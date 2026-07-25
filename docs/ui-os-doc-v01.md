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
| ONLINE / OFFLINE | green / red | Tiny bridge dot only |

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

Typing a natural ask on Home and pressing Enter runs the guest plan/act loop
(`agent::run_goal`). The host `intent.resolve` tool supplies the smart plan
(act + expanded query + ranked `file://` hits when Your files is on). The
guest arms **Doc** rows and may still CALL `search.query`. Example:
`i wanna work on my paper` → open act → paper/thesis/draft query → workspace
rank → Reader. No model in the kernel — missing caps stay Need.

## Recent mail (v0.9.22)

Home lists recent inbox rows when Email is granted. Each row is clickable and
opens `email://{id}` in the Reader — same `doc.read` path as Search. The id
comes from the bridge `email.search` ROW (graph-stable hash of from+subject).

## Recent files (v0.9.25)

Home lists top-ranked workspace files when Your files is granted. Each row is
clickable and opens `file://…` in the Reader (`doc.read` + `files=1`). Rows
come from bridge `workspace.recent` after `workspace.index`.

## Calendar events (v0.9.24)

Morning / inbox Briefs list upcoming events when Email is on. An Event row is
clickable and opens `cal://{id}` in the Reader (`doc.read` + `email=1`). Ids
come from `calendar.list` ROWs (hash of title+when).

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
