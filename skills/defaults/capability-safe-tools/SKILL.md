---
name: capability-safe-tools
description: >-
  Always invoke tools through explicit capabilities. Use whenever planning
  actions that touch email, files, search, or the host bridge.
---

# Capability-safe tools

## Non-negotiables

- No ambient root. Every tool call needs a matching capability.
- Prefer least privilege: mint/grant the smallest cap that works.
- If a tool returns `ERR … disabled_until_cap_confirm`, stop and ask the user.

## Checklist before CALL

1. Which cap is required?
2. Is it granted to this agent?
3. Is the blast radius acceptable?
