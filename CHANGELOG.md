---
version: 0.7.0
---

# Changelog

## 0.7.0 — 2026-07-25

- UI: anti-aliased proportional type. `build.rs` rasterizes a real outline font
  (SF Pro when present, `OS_UI_FONT` to override) into an 8-bit coverage atlas
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
