---
name: knowledge-search
description: >-
  Search the curated OS knowledge corpus via search.query (tSearch-style
  tf-idf × PageRank). Use when the agent needs docs, skills, or concepts —
  not for live email (use email-triage).
---

# Knowledge search

Inspired by [tsearch-revival](https://github.com/theoko/tsearch-revival): lexical
ranking over `{t,u,c,b,pr}` docs, exact-first ladder, no crawler in v1.

## Rules

1. Call `search.query` with a short keyword query (`q=`). Optional `k=` (1–20;
   omit for the guest default of 3).
2. Cite `title` + `url` from each ROW.
3. Prefer corpus hits over guessing architecture/caps policy.
4. Offline corpus is baked into the guest; the host bridge merges teddy + files
   (lazy index) + host-indexed transcripts when those caps are granted. Audio
   hits stay empty until the host has run `audio.transcribe path=…`.

## Flow

1. `CALL search.query q=capability ambient`
2. Skim ROW title/cat/url
3. If thin, refine the query keywords
4. Act using the cited source (plan, etc.)
