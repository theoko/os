---
name: agent-plan-act
description: >-
  Default plan/act loop for os agents: clarify goal, list caps, call tools,
  report. Home Enter runs this against the typed ask.
---

# Plan / act

1. Restate the goal in one sentence.
2. List required capabilities.
3. Plan ≤5 steps.
4. Act via MCP connectors / skills only (search, files, mail under grants).
5. Report outcomes; arm Doc / Event rows the user can open.
6. Remaining risks: missing grants stay `Need` — never invent hits.

## Home field

Natural language on Home (`i wanna work on my paper`) is not bare keyword
search. The guest CALLs host `intent.resolve` for act classification, synonym
expansion, and workspace ranking, then executes under caps and opens a Brief
with Doc rows. Optional cloud LLM can back the same tool later; the ISO never
runs a model.
