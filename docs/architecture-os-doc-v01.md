---
project: teddyOS / os
type: architecture
purpose: agent-centric OS design north star
status: active — aspirational; see tip note
---

# Architecture — agent-centric OS (v01)

> **Tip note (teddyOS / `fix/bridge-prewarm`).** This doc is the long-range
> north star (`agentd` + capability IPC). **Shipped shape on this tip** is
> freestanding kernel + Linux live (`linux/`) + baked corpus — **no** host MCP
> bridge / COM2 connectors. MCP `main` is the separate bridge product. See
> `STATUS.md` and `AGENTS.md`.

## Intent

Build a Rust x86_64 hobby OS whose **unit of authority is the agent**, not a traditional all-powerful root process. Chat UI is a thin client. LLM inference is a userspace service (later), never in-kernel.

## Layers

```
Chat UI  →  agentd (plan/act)  →  IPC bus + capabilities  →  sandboxed tools / services
                                      ↑
                                   kernel
```

### Kernel

Owns:

- Boot (Limine), memory, interrupts, scheduling
- Per-process **capability tables**
- **IPC message bus** (`send` / `recv`) that can transfer or grant caps by reference

Does **not** own: LLM weights, HTTP clients, chat rendering.

### Userspace

- **`agentd`** — receives goals, plans steps, invokes tools only through granted caps
- **Tools** — one process per tool invocation (or long-lived tool daemon) with a *minimal* capability set
- **Services** — filesystem, console, later inference gateway
- **Chat UI** — speaks only to `agentd`

## Capability model (non-negotiable)

1. No ambient root for agents or tools.
2. Holding a capability is the only way to exercise an authority (open this directory, write this serial port, talk to that service endpoint).
3. The kernel checks the capability token on every relevant syscall/IPC.
4. Caps can be **minted**, **granted** (delegate), and **dropped**; messages may carry cap rights.

Planned syscall sketch (names may change before Phase 3):

| Call | Role |
|------|------|
| `send` / `recv` | Message bus |
| `mint_cap` | Create a capability of a kernel-known type |
| `grant` | Delegate a cap (or subset) to another process |
| `drop_cap` | Relinquish |

## Phased delivery

| Phase | Deliverable |
|-------|-------------|
| 1 | Limine boot, serial hello in QEMU |
| 2 | Memory, IDT/timer, kernel threads |
| 3 | Ring-3 userspace + capability IPC |
| 4 | `agentd` + sandboxed tools |
| 5 | Thin chat UI |
| standalone (this tip) | No host MCP / COM2 connectors; corpus baked into the ISO; Linux live apps under `linux/` |
| bridge (MCP `main` only) | Host MCP connectors (email via COM2); not landed on this fork |

## Explicit non-goals (v0.x)

- Linux ABI compatibility
- In-kernel networking / TLS / OAuth
- In-kernel inference
- Production multi-user security audit (hobby correctness first)
