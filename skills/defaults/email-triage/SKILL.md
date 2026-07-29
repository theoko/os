---
name: email-triage
description: >-
  Placeholder for inbox triage. Offline in standalone builds — needs a future
  userspace network connector behind Cap::EmailSearch. Do not invent mail.
---

# Email triage (offline)

Standalone images have no host bridge and no network stack. `email.search`
and `email.send` always return offline.

When a real connector exists again:

1. Search with a narrow query (cap required).
2. Summarize; never invent message contents.
3. Draft replies only; send needs Send-mail cap **and** explicit confirm.
4. Secrets stay out of the guest and off the ISO.
