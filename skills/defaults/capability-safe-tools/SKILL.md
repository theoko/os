---
name: capability-safe-tools
description: >-
  Always act through explicit capabilities. Use whenever planning actions that
  touch email, files, calendar, search, or other gated tools.
---

# Capability-safe tools

## Non-negotiables

- No ambient root. Every tool action needs a matching capability.
- Prefer least privilege: grant the smallest cap that works.
- If a tool returns offline or needs a grant, stop and ask the user.
- On standalone images there is no host connector to “start” — Offline means
  unavailable on this device, not a missing `make` command.

## Checklist before acting

1. Which cap is required?
2. Is it granted to this agent?
3. Is the blast radius acceptable?
4. If Offline: report that honestly; do not invent results.
