---
name: teddy-portals
description: >-
  Teddysearch.com corpus + live portal tools. Live HTTPS tools are offline in
  standalone builds; corpus hits come from the baked ISO index when granted.
---

# Teddy API + portals (standalone)

## Corpus (works offline)

Search answers from `search/corpus.json` compiled into the kernel. Prefer the
knowledge-search skill for that path.

## Live portals (offline for now)

HTTPS tools (`teddy.health`, `teddy.fear_greed`, `teddy.gex`) need a guest
network stack and `Cap::PortalSync`. Until then, report offline — do not invent
scores.
