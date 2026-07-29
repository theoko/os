# linux/ — teddyOS desktop on a Linux live image

This directory is the **daily-driver** path for the standalone tip: a Debian
live image plus teddyOS apps. It is **not** a guest for a host MCP bridge.

On this tip (`AGENTS.md` / `STATUS.md`):

- No `host/bridge` in the tree
- No COM2 host connector — freestanding / offline stubs only
- Non-technical audience first

## What ships

| Piece | Role |
|-------|------|
| `iso/` + `make linux-iso` | Build the live ISO (`scripts/build-linux-iso.sh`) |
| `teddyos-search/` | In-guest search over baked corpus / granted sources |
| `teddyos-setup/` | First-run / capabilities UI on the desktop |
| `teddyos-claude/` | Launch Claude in a project folder |
| `teddyos-update/` | Update helpers |
| `shell-extension/` | GNOME shell bits |
| `logging/` | Journal / collect helpers |
| `icons/` | App icons |

Build:

```sh
make linux-iso                 # native arch of this machine
make linux-iso ARCH=amd64      # cross-build when needed
```

See `linux/iso/build-iso.sh` and `scripts/build-linux-iso.sh`.

## Search from the desktop

```sh
teddyos-search wheel strategy
teddyos-search --caps
teddyos-search --json nvda
```

Exit codes matter: blocked/denied must never look like “no matches.” See the
header comment in `teddyos-search/teddyos-search`.

## Related docs

- [`AGENTS.md`](../AGENTS.md) — identity / non-negotiables for this tip
- [`STATUS.md`](../STATUS.md) — review-only fork vs MCP `main`
- [`docs/linux-os-doc-v02.md`](../docs/linux-os-doc-v02.md) — proposal history (not live wiring)
