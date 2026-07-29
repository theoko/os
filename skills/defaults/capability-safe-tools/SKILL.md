---
name: capability-safe-tools
description: >-
  Cap discipline for personal-data and write tools. Use when planning actions
  that touch email, files, calendar, portals, or skills.save — not for ungated
  catalog calls (skills.list / skills.get).
---

# Capability-safe tools

## Non-negotiables

- No ambient root for personal data or writes. Those CALLs need a matching Cap
  (and wire bit): mail, files, audio, portals, `skills.save`, `email.send`.
- Prefer least privilege: mint/grant the smallest Cap that works.
- Catalog peeks are ungated: `skills.list` / `skills.get` need no wire bit.
  Curated `os://` `doc.read` needs no personal-data bit. Guest may still refuse
  `search.query` without Knowledge.
- If a tool returns `ERR … disabled_until_cap_confirm`, stop — for `email.send`
  that means Confirm send on Brief (`confirm=1`), not inventing a second Cap.

## Checklist before CALL

1. Is this Cap-gated, ungated catalog, or confirm-gated (`email.send`)?
2. If gated: which Cap / wire bit, and is it granted?
3. Is the blast radius acceptable?
