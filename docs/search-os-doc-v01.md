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

## Host backends

| Env | Behavior |
|-----|----------|
| `OS_MCP_SEARCH_BACKEND=tfidf` (default) | In-bridge tf-idf × (1+4·pr), exact-AND bonus |
| `OS_MCP_SEARCH_BACKEND=mock` | Fixed demo rows (CI without corpus) |
| `OS_MCP_SEARCH_BACKEND=tsearch` | Shells to `TSEARCH_DATA/tsearch_mcp.py` |

Override corpus path with `OS_SEARCH_CORPUS`.

## Skill

`knowledge-search` — playbook for agents; see `skills/defaults/knowledge-search/SKILL.md`.

## Non-goals (v1)

- No in-kernel index or networking
- No web crawler, LSA, Louvain, or GraphRAG edges
- No secrets in `search/corpus.json`
