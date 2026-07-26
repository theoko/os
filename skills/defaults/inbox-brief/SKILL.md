---
name: inbox-brief
description: >-
  Produce a short morning brief from email.search results for the home UI /
  chat client. Use when the user asks what matters in mail right now.
---

# Inbox brief

`email.search` returns a peek count only (`ROW n=<count>`) — no subject/from
payloads on the wire.

Format (keep it tight):

- **Count** — `n` from `ROW n=`
- **Signal** — high / quiet / empty relative to the user's usual inbox
- **Next** — one suggested action (open Gmail on the host, grant caps, wait)

Do not invent message contents. Max 5 bullets.
