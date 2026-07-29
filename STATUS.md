---
project: teddyOS / os
purpose: status for the standalone 0.13 tip
status: active on fix/bridge-prewarm — not a land candidate for MCP main
---

# STATUS — `fix/bridge-prewarm` / teddyOS 0.13

## What this tip is

A **separate product tip** (changelog **0.13.0**) on `origin/fix/bridge-prewarm`.

Standalone honesty review landed via [#21](https://github.com/theoko/os/pull/21)
(+ [#22](https://github.com/theoko/os/pull/22) STATUS follow-up). Product work
continues on this tip (e.g. work-on AI picker / “Ask every ready AI”).

Shape (see `AGENTS.md` on this tip):

- Freestanding kernel + **Linux** live image (`linux/`) + teddyOS desktop apps
- **Standalone** — no host MCP bridge, no COM2 connector
- Non-technical audience is a non-negotiable
- Work-on goals, AI tool picker, durable guest journals, GitHub clone for
  projects, plain-language UI pass

Tip: see `git log -1` on `fix/bridge-prewarm`.

## SAFE densify floor (docs + standalone copy)

Honesty cuts on this tip are at a **SAFE floor** for:

- Design docs (search/skills/architecture/thesis/linux/install/utm/…)
- Baked corpus (no `os://host/bridge` seed rows)
- User-visible standalone status/remedy copy (`kernel/src/copy.rs` + status
  bar offline lines) — never tell owners to `make bridge-run` / `utm-bridged`
- Default skill playbooks (`agent-plan-act`, `capability-safe-tools`, …)
- E2E selfcheck fixtures use standalone copy (`Local keywords only.`), not
  hosted “Bridge offline …” strings

**Do not invent** without a product brief: restoring Live COM2 connectors,
merging this tip into MCP `main`, or new `email.send` confirm UX.

## What this tip is not

- **Not** a merge candidate for bridge-centric `main` (0.10.0 MCP guest + host
  bridge on `:7420`).
- **Not** a continuation of the densify / `agent.act` honesty line on `main`.
- Opening a PR **into `main`** from this tip would be the wrong product story
  and a large conflict bomb (~50 commits / different tree shape).

## All-branches review — complete

| Tip | State |
|-----|--------|
| MCP `main` | **0.10.0** + Cap / `agent.act` / `email.send` honesty (#15–#20, #23). Soft leftover comment fixed. |
| `fix/bridge-prewarm` | Standalone teddyOS 0.13; honesty densify landed (#21–#22). Product tip continues here. |
| Dial / obsolete drafts | #9–#14 closed or won't-merge (dial vs listen). |

No open land PRs remain from this review. Next product work needs an explicit
brief (new feature on either tip, archive this fork, or deliberate port into
MCP `main`) — not more densify.
