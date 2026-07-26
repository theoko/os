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
| Agent path | tf-idf × PageRank, exact-then-synonym ladder |

**os v1** follows the **agent** path, not the crawler/LSA/GraphRAG stack.

## Shape

```
guest SearchView
  ├─ bridge online  -- CALL search.query -->  host/bridge
  │                        search/corpus.json + teddy / files / audio when granted
  └─ offline / empty     baked kernel index (search/corpus.json via build.rs)
```

Corpus docs use the same card fields as tSearch: title `t`, url `u`, category `c`,
body `b`, optional PageRank-ish boost `pr`.

## Protocol

| Call | Result |
|------|--------|
| `CALL search.query q=… [k=n]` | `OK search.query` + `ROW title=…\|cat=…\|url=…` + `END` |

Guest omits `k=` (host default 3 = guest `MAX_HITS`; wire max 20). In-bridge
ranking is tf-idf × (1+4·pr) with an exact-AND bonus. Override corpus path with
`OS_SEARCH_CORPUS`.

## Skill

`knowledge-search` — playbook for agents; see `skills/defaults/knowledge-search/SKILL.md`.

## Non-goals (v1)

- No in-kernel networking (offline index is baked by `build.rs`)
- No web crawler, LSA, Louvain, or GraphRAG edges
- No secrets in `search/corpus.json`
