---
name: capability-safe-tools
blurb: Least-privilege caps
description: >-
  Always invoke tools through explicit capabilities. Use whenever planning
  actions that touch email, files, search, or the host bridge.
---

# Capability-safe tools

## Non-negotiables

- No ambient root. Cap-gated guest CALLs (Inbox/Knowledge and `files=1` /
  `audio=1` scopes) need a matching grant before you CALL.
- Prefer least privilege: mint/grant the smallest Cap that works.
- Some bridge tools are not Cap-gated (`skills.list`) or are policy stubs
  (`email.send` → always `ERR … disabled_until_cap_confirm`). Do not invent a
  Cap for those — stop on the stub; use `skills.list` freely.

## Checklist before CALL

1. Is this Cap-gated, ungated, or a policy stub?
2. If gated: which Cap, and is it granted?
3. Is the blast radius acceptable?
