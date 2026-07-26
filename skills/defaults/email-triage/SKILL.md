---
name: email-triage
description: >-
  Triage inbox via the email MCP connector's peek count. Use when the agent
  should check how full the inbox is — never send without an explicit
  email.send cap.
---

# Email triage

## Rules

1. Call `email.search` (cap required). Guest peeks with a bare CALL; host may
   accept optional `q=` / `max=` for nc/gog.
2. Read `ROW n=<count>` only — no per-message payloads on the wire. Do not
   invent subjects, senders, or bodies.
3. Propose next actions from the count (open host Gmail, wait, draft offline).
   `email.send` stays blocked until the user grants confirm.
4. Secrets and OAuth stay on the host bridge (`gog` keyring) — never ask to
   paste tokens into the guest.

## Flow

1. `CALL email.search`
2. Note `ROW n=`
3. Propose next actions (check host inbox / draft / wait)
