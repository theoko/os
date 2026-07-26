---
project: os
type: skills
purpose: agent skills registry (defaults + saved)
status: active
---

# Skills (v01)

Skills are Cursor-style `SKILL.md` packages the agent loads before acting.
They are **not** kernel code. Defaults ship in-repo; user saves live on the host.

## Layout

```
skills/defaults/<name>/SKILL.md     # shipped with the OS repo
~/Library/Application Support/os/skills/<name>/SKILL.md   # saved / user
```

Frontmatter: `name`, optional `blurb` (≤40 chars — guest Skills row; matches
ISO builtins), `description` (agent routing prose; Cursor-style).
`skills.list` prefers `blurb` when present, else truncates `description`.

## Bridge API

| Call | Result |
|------|--------|
| `CALL skills.list` | `OK skills.list` + `ROW name=…\|desc=…` + `END` |
| `CALL skills.save name=…` | writes under Application Support (body via `LINE`…`END` after request) |

Guest always knows **builtin** defaults even if the bridge is offline.

## Defaults

| Skill | Role |
|-------|------|
| `agent-plan-act` | Baseline plan/act loop |
| `capability-safe-tools` | Cap discipline |
| `email-triage` | Inbox peek count via MCP email connector |
| `inbox-brief` | Short mail brief from peek count |
| `knowledge-search` | Curated corpus via `search.query` (tSearch-style) |

Also see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).
