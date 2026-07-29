---
project: teddyOS / os
purpose: review-only status for the standalone 0.13 fork
status: parked — not a land candidate for MCP main
---

# STATUS — `fix/bridge-prewarm` / teddyOS 0.13

## What this branch is

A **separate product tip** (changelog **0.13.0**), currently tracked as
`origin/fix/bridge-prewarm` and mirrored here as
`cursor/teddyos-0.13-review-492f` for review visibility.

Shape (see `AGENTS.md` on this tip):

- Freestanding kernel + **Linux** live image (`linux/`) + teddyOS desktop apps
- **Standalone** — no host MCP bridge, no COM2 connector
- Non-technical audience is a non-negotiable
- Work-on goals, AI tool picker, durable guest journals, GitHub clone for
  projects, plain-language UI pass

Tip at review open: `bb716aa` (`fix(ui): plain-language pass for non-technical users`).

## What this branch is not

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
on its own line until a product decision chooses:

1. Continue as teddyOS 0.13+ (own release / own default branch story), or
2. Explicitly abandon / archive, or
3. A deliberate (non-ff) port of selected ideas into MCP `main` under a brief.

## How to review

```sh
git fetch origin
git checkout cursor/teddyos-0.13-review-492f   # or origin/fix/bridge-prewarm
# Read AGENTS.md, CHANGELOG 0.13.0, linux/README.md
```

Do not `git merge` this into `main` without an explicit product brief.
