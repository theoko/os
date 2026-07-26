---
name: email-triage
description: >-
  Triage inbox via the email MCP connector. Use when the agent should search,
  summarize, or draft replies — never send without an explicit email.send cap.
---

# Email triage

## Rules

1. Search with `email.search` (cap required). Prefer narrow Gmail queries.
2. Summarize; do not invent message contents.
3. Draft replies only; `email.send` needs the Send mail cap **and** an
   explicit Confirm send on Brief (`confirm=1`). Never auto-CALL send.
4. Secrets and OAuth stay on the host bridge (`gog` keyring) — never ask to paste tokens into the guest.

## Flow

1. `CALL email.search q=… max=5`
2. Rank by urgency
3. Propose next actions (archive / draft / wait)
