---
project: os
type: architecture
purpose: knowledge search (tSearch-inspired) via host bridge
status: active
---

# Knowledge search (v01)

## Inspiration

[tsearch-revival](https://github.com/theoko/tsearch-revival) exposes two paths:

| Path | Ranking |
|------|---------|
| Browser (`index.html`) | Hybrid BM25 + LSA + PageRank |
| Agent MCP (`tsearch_mcp.py`) | FTS5 BM25 or tf-idf × PageRank, exact-then-synonym ladder |

**os v1** follows the **agent** path, not the crawler/LSA/GraphRAG stack.

## Shape

```
guest  -- CALL search.query -->  host/bridge
                                      |
                         search/corpus.json  (curated {t,u,c,b,pr})
                         optional TSEARCH_DATA → tsearch_mcp.t_search
```

Corpus docs use the same card fields as tSearch: title `t`, url `u`, category `c`,
body `b`, optional PageRank-ish boost `pr`.

## Protocol

| Call | Result |
|------|--------|
| `CALL search.query q=… k=5 cat=docs` | `OK search.query n=N backend=…` + `ROW title=…\|cat=…\|score=…\|snip=…\|url=…` + `END` |
| `… email=1` / `files=1` / `audio=1` / `portal=1` | Opt-in personal sources (mail graph, workspace, transcripts, teddy corpus) |

Guest Search empty-state names the next missing grant in order: workspace →
email → audio. The home Search tile subtitle lists `docs` / `mail` / `files` /
`online` / `audio` from current caps. Opening a hit uses `doc.read` with the
matching wire bit; curated `os://` bodies need none. Morning `plan_act` notes
Your files / Recordings when those grants are on (open via Search). With
Recordings on, typing an absolute media path and pressing Enter runs
`audio.transcribe` (host path picker) before the usual query.

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_SEARCH_BACKEND=tfidf` (default) | In-bridge tf-idf × (1+4·pr), exact-AND bonus |
| `OS_MCP_SEARCH_BACKEND=mock` | Fixed demo rows (CI without corpus) |
| `OS_MCP_SEARCH_BACKEND=tsearch` | Shells to `TSEARCH_DATA/tsearch_mcp.py` |

## Teddy API vs teddy portals

| Path | Tools | What it is |
|------|-------|------------|
| **Teddy API** | `tsearch.sync`, `search.query … portal=1` | Cached `corpus.json` ranked locally |
| **Teddy portals** | `teddy.health`, `teddy.fear_greed`, `teddy.gex` | Live HTTPS JSON on teddysearch.com |
| **Market portals** | `market.health`, `market.fear_greed` | Live HTTPS JSON on superintelmarkets.com |

Teddy API, teddy portals, and market portals all require `portal=1` / guest
`Cap::PortalSync`. Corpus hits also need `search.query`. See skills
`teddy-portals` and `market-portals`.

Override corpus path with `OS_SEARCH_CORPUS`.

## Skill

`knowledge-search` — playbook for agents; see `skills/defaults/knowledge-search/SKILL.md`.
`teddy-portals` — corpus API plus live teddysearch.com tools under the same grant.
`market-portals` — live superintelmarkets.com tools under the same grant.

## Non-goals (v1)

- No in-kernel index or networking
- No web crawler, LSA, Louvain, or GraphRAG edges
- No secrets in `search/corpus.json`
