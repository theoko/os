---
project: teddyOS / os
purpose: status for the Linux-only tip
status: active on fix/bridge-prewarm
---

# STATUS — Linux-only teddyOS

## What this tip is

The **Linux daily driver** product: `linux/` apps + live ISO + UTM desktop path.

As of **v0.15.0** the freestanding Rust kernel (`kernel/`, Limine, `os.iso`,
UTM VM `os`, VirtualBox freestanding ARM) was **removed** from the tree.
It was noise next to the product operators actually use.

## Run

```sh
make linux-init && make linux-provision   # first time
make desktop                              # every day
make test                                 # Linux contracts
```

## What this tip is not

- Not a freestanding hobby kernel
- Not a merge story into an old MCP/bridge `main` without a product decision
