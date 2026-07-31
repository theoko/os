---
project: os
purpose: local intent router for Search (agent decides, product executes)
status: active
version: v01
date: 2026-07-30
---

# Intent router — Search (v01)

## Job

Turn whatever a nontechnical person types into Search into a **structured
intent**, then run a **known playbook**. No free-roaming agent. Never auto-send.

```
Search text → intent.classify() → Intent { kind, channel, confidence, summary }
           → Search dispatches playbook
           → if ambiguous: plain choice rows (user pick → force intent)
```

## Kinds

| Kind | Product |
|------|---------|
| `message_reply` | Open LinkedIn / WhatsApp + AI draft playbook |
| `project_help` | Resolve folder / GitHub / Get help |
| `freeform_help` | Get help against home (email-ish tasks) |
| `lookup` | Corpus + web (default search) |
| `ambiguous` | Offer 2–3 plain choices |
| `setup` | Reserved |

## Files

- `linux/teddyos-search/intent.py` — pure rules classifier (offline, testable)
- `linux/teddyos-search/teddyos-search-app` — `_work_goal_block` → dispatch
- ISO installs `intent.py` to `/usr/lib/teddyos/`

## v1 rules only

High confidence for explicit LI/WA. Generic “reply to my messages” → ambiguous
chips (WhatsApp / LinkedIn / Just search). Project vs freeform uses existing
`search.is_*` helpers + slug heuristics.

## Future

`source="model"` path: when signed in and `kind=ambiguous`, ask one AI to pick
from the fixed IntentKind list (JSON only). Same dispatcher — no new product
surface.
