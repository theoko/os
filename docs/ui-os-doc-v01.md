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

## Code

- [`kernel/src/ui.rs`](../kernel/src/ui.rs)
- [`kernel/src/fb.rs`](../kernel/src/fb.rs)
