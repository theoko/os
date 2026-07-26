---
name: teddy-portals
description: >-
  Use teddysearch.com two ways: the corpus API (tsearch.sync + search.query
  with portal=1) and live portal tools (teddy.health, teddy.fear_greed,
  teddy.gex). Requires portal.sync; corpus hits also need search.query.
---

# Teddy API + portals

teddysearch.com is not one connector. Keep the two paths distinct.

## Teddy API (corpus)

There is no server-side query endpoint. The published file
`https://teddysearch.com/tsearch/corpus.json` *is* the API.

1. `CALL tsearch.sync portal=1` — fetch/validate/cache the corpus (once).
2. `CALL search.query q=… k=5 portal=1` — local tf-idf × PageRank over that
   cache (plus built-in docs when `search.query` is granted).

## Teddy portals (live)

HTTPS JSON tools on the same host. Every call leaves the machine:

| Tool | URL |
|------|-----|
| `teddy.health` | `/health` |
| `teddy.fear_greed` | `/api/fear-greed` (`ticker`, `purpose` optional) |
| `teddy.gex` | `/api/gex` |

Always pass `portal=1`. Without it the bridge returns `needs_portal_cap`.

## Flow

1. Confirm `portal.sync` (and `search.query` if you need corpus hits).
2. Sync once if the cache is cold.
3. Search the corpus; cite `title` + `url` only.
4. Call `teddy.health` then `teddy.fear_greed` / `teddy.gex` for live fields.
5. Do not invent scores — only report `ROW field=…|value=…` from the bridge.
