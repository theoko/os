# linux/ — teddyOS (the product)

Debian live image + desktop apps. This directory **is** the OS for daily use.

## Run (Apple Silicon)

```sh
make help
make linux-init          # once — Debian aarch64 disk + cloud-init
make linux-provision     # once — GNOME + Chromium (headless QEMU, ssh :2222)
make desktop             # open the Linux guest in UTM (VM name: teddyos)
```

## What ships here

| Piece | Role |
|-------|------|
| `iso/` + `make linux-iso` | Live ISO build |
| `teddyos-search/` | Search, intent router, work-on / messaging |
| `teddyos-setup/` | First-run / capabilities |
| `teddyos-agent/` | Display, LinkedIn, WhatsApp, accounts |
| `teddyos-update/` | In-guest updates |
| `shell-extension/` | GNOME shell bits |
| `logging/` | Journal / collect |
| `icons/` | App icons |

```sh
make linux-iso                 # native arch of the guest
make linux-iso ARCH=amd64      # cross-build when needed
make test                      # host contract unit tests
```
