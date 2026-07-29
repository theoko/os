---
project: teddyOS / os
type: architecture
purpose: knowledge search on the standalone tip
status: active — this tip; not the MCP main bridge story
---

# Knowledge search (v01) — standalone tip

> **Tip note.** This document describes **teddyOS / `fix/bridge-prewarm`**,
> where search is **in-kernel** from a baked corpus. It is **not** the MCP
> `main` story (`CALL search.query` → `host/bridge` + `OS_MCP_SEARCH_BACKEND`).
> See `STATUS.md`.

## Inspiration

[tsearch-revival](https://github.com/theoko/tsearch-revival): lexical ranking
over curated cards `{t,u,c,b,pr}`. This tip keeps the **agent ranking idea**,
not the crawler / LSA / GraphRAG stack, and not a host MCP process.

## Shape (this tip)

```
guest UI / agent
      │
      ▼
kernel/src/search.rs   ← static index from search/corpus.json (build.rs)
      ▲
scripts/bake-corpus.py ← writes corpus.json from search/seed.json
```

There is **no** `host/bridge`, **no** COM2 search connector, and **no**
`OS_MCP_SEARCH_BACKEND`. When MCP stubs report Offline, `mcp::fetch_search_peek`
falls through to the same local index (`search_offline`).

Corpus fields match tSearch cards: title `t`, url `u`, category `c`, body `b`,
optional PageRank-ish boost `pr`. Publish / CI bake from seed only:

```sh
./scripts/bake-corpus.py --seed-only
```

Personal corpus must never ship via `make publish-os` (see `AGENTS.md`).

## What still needs a grant

Guest Search under the freestanding kernel still respects `Cap::SearchQuery`
for corpus hits. Personal sources (mail graph, host workspace files, live
portals) are **offline** on this tip — stubs return empty / Offline; there is
no host process to satisfy them.

## Skill

`knowledge-search` — playbook for agents; see
`skills/defaults/knowledge-search/SKILL.md`. Portal / market playbooks that
assume live HTTPS remain docs-only until a guest network stack exists.

## Non-goals (this tip)

- No in-kernel networking or live web crawl
- No host MCP search backends
- No secrets in `search/corpus.json` / `seed.json`
