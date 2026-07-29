---
project: teddyOS / os
purpose: status for the standalone 0.13 tip
status: active on fix/bridge-prewarm — not a land candidate for MCP main
---

# STATUS — `fix/bridge-prewarm` / teddyOS 0.13

## What this tip is

A **separate product tip** (changelog **0.13.0**) on `origin/fix/bridge-prewarm`.

Standalone honesty review landed via [#21](https://github.com/theoko/os/pull/21)
(`cursor/teddyos-0.13-review-492f` → this branch).

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

## Relationship to `main` (all-branches review)

After the 0.10.0 integration land (#15) and Cap/`agent.act` honesty follow-ups
(#16–#20), MCP `main` is the live bridge tip. Drafts #9–#14 were closed or
parked; dial topology (#14) was won't-merge against listen-mode.

This fork remains the **standalone / Linux daily-driver** experiment. Keep it
on `fix/bridge-prewarm` (or a renamed long-lived tip) until an explicit
product decision: continue, archive, or deliberate port into MCP `main`.
