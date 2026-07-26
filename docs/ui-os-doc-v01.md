---
project: os
type: ui
purpose: framebuffer home screen (launcher)
status: active
---

# UI — home screen

## Direction

Home is a **launcher**, not a landing page: type into search on arrival, three
destination tiles (Search, Capabilities, Skills), inbox count in the footer
when `email.search` is granted.

## Palette

| Token | Hex | Role |
|-------|-----|------|
| BG | `#F5F5F7` | Page (`fb::PAGE_BG`) |
| INK | `#1D1D1F` | Primary type |
| MUTED | `#86868B` | Secondary copy |
| ACCENT | `#0071E3` | Actions |
| ONLINE / OFFLINE | green / red | Bridge status dot |

## Composition

1. Nav: brand `os` + Connect + bridge status
2. Search field (type immediately; Enter runs Search)
3. Three tiles (Search static; Capabilities / Skills show live counts)
4. Footer: inbox count, active caps, bridge status

Capabilities rows use short human labels (`Inbox`, `Knowledge`, `Files`,
`Transcripts`); host-tool verbs stay on the wire / COM1 revoke lines.

Home and list screens share one content column cap (`CONTENT_MAX` = 720) so the
search field width does not jump when Enter opens Search.

**Constraint:** proportional Inter atlas is ASCII only (`0x20..=0x7E`).
Never put em dashes, middots, or curly quotes in on-screen strings — they become `?`.

## Code

- [`kernel/src/ui.rs`](../kernel/src/ui.rs)
- [`kernel/src/screens.rs`](../kernel/src/screens.rs)
- [`kernel/src/searchui.rs`](../kernel/src/searchui.rs)
- [`kernel/src/fb.rs`](../kernel/src/fb.rs)
