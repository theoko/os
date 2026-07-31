---
project: os
purpose: identity card for agents and humans
status: active
updated: 2026-07-31
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
make desktop
make linux-iso
make test
```

## NON-NEGOTIABLES

1. **Linux is the only product.**
2. **Non-technical first.**
3. **Capability model** — messaging open + draft only, never auto-send.
4. **No secrets in the tree.**
5. **Tests gate commits** — `make test` for `linux/` changes.
6. **Version + changelog** on real work.

## Ship a change

1. Implement + `make test`
2. Bump `VERSION` + `CHANGELOG.md`
3. Conventional commit
4. `make desktop` to check interactively
