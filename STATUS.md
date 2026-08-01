---
project: teddyOS / os
purpose: status for the Linux-only tip
status: active on main
version: 0.16.3
updated: 2026-08-01
---

# STATUS — Linux-only teddyOS

## What this tip is

The **Linux daily driver** product: `linux/` apps + live ISO + UTM desktop path.
**main @ v0.16.3** after PR #25 (brand, dock, Connect/answers, GitHub work-on,
full product desktop matrix).

## Run

```sh
make linux-init && make linux-provision   # first time (substrate)
make linux-product                        # Search/dock/theme into a guest
make desktop                              # every day (UTM)
make test                                 # Linux contracts (host)
```

## Guest notes (UTM)

- Brand: `teddyos-brand.sh` (GDM logo off, os-release teddyOS)
- Dock: Dash to Dock fixed; Just Perfection **dash=true** (required)
- GitHub: `gh` installed; public `owner/repo` clones without login; private
  repos need **Connect GitHub** once (`gh auth login` via teddyos-accounts)
- Screen lock → extensions report INACTIVE until unlock (expected)

## What this tip is not

- Not a freestanding hobby kernel (removed v0.15.0+)
- Not an MCP host-bridge product path
