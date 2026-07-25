---
name: market-portals
description: >-
  Call live market portals on superintelmarkets.com (market.health,
  market.fear_greed) under portal.sync. Use for status and fear/greed —
  not for the teddysearch.com corpus (use teddy-portals).
---

# Market portals

Same consent bit as teddy portals (`portal=1` / guest `Cap::PortalSync`),
different origin.

| Tool | URL |
|------|-----|
| `market.health` | `https://superintelmarkets.com/health` |
| `market.fear_greed` | `https://superintelmarkets.com/api/fear-greed` |

## Rules

1. Always pass `portal=1`. Without it the bridge returns `needs_portal_cap`.
2. Report `ROW field=…|value=…` only — do not invent scores.
3. Cache is ~120s on the host; do not hammer the endpoint per keystroke.
4. For teddy corpus + teddysearch.com live tools, use `teddy-portals`.

## Flow

1. Confirm Online services is on.
2. `CALL market.health portal=1`
3. `CALL market.fear_greed portal=1` (optional `ticker=`, `purpose=`)
4. Summarize status / degraded services / fear-greed components.
