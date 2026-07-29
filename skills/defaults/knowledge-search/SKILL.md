---
name: knowledge-search
description: >-
  Search the curated OS knowledge corpus baked into the ISO (tSearch-style
  tf-idf × PageRank). Use when the agent needs docs, skills, or concepts.
---

# Knowledge search

Inspired by [tsearch-revival](https://github.com/theoko/tsearch-revival): lexical
ranking over `{t,u,c,b,pr}` docs, exact-first ladder. The freestanding kernel
answers only from the corpus compiled into the image — no host bridge, no live
network fetch.

## Rules

1. Prefer short keyword queries.
2. Cite `title` + `url` from each hit; do not invent snippets.
3. Prefer corpus hits over guessing architecture or caps policy.
4. Email and live portals are offline until a guest network stack exists.

## Flow

1. Search for a short phrase (e.g. capability ambient).
2. Skim titles and sources.
3. If thin, refine the query or narrow by category.
4. Act using the cited source.
