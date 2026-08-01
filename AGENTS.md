---
project: os
purpose: identity card for agents and humans
status: active
updated: 2026-08-01
version_note: Ship version is the top-level VERSION file.
---

# os (teddyOS)

**Linux daily driver only.** Debian + teddyOS desktop apps under `linux/`,
opened with `make desktop` (UTM on Apple Silicon).

There is **no freestanding Rust kernel** in this tree.

When you land here: read this file, then `VERSION` + newest `CHANGELOG.md`
entry, then work under `linux/`.

## Shape

```
linux/           product (setup, search, agents, update, live ISO)
scripts/         desktop / UTM / provision / live-ISO / publish
docs/            Linux design notes
search/          seed corpus optional for live ISO bake
Makefile         default = make desktop
```

## Commands

```sh
make help
make linux-init
make linux-provision
make linux-product
make desktop
make linux-iso
make test
```

## NON-NEGOTIABLES

1. **Linux is the only product.** No freestanding kernel / “hello” ISO.
2. **Non-technical first.** Plain language on every user-facing surface.
3. **Capability model.** Tools denied until granted. Messaging = open + draft only (never auto-send).
4. **No secrets in the tree.**
5. **Tests gate commits.** `make test` (Linux contracts) when touching `linux/`.
6. **Version + changelog on real work.** Bump `VERSION` + newest-first `CHANGELOG.md`.

## Ship a change

1. Implement + `make test`
2. Bump `VERSION` + `CHANGELOG.md`
3. Conventional commit
4. `make desktop` to check interactively
