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

Frontmatter: `name`, `description` (same convention as Cursor skills).

## Bridge API

| Call | Result |
|------|--------|
| `CALL skills.list` | `OK skills.list n=N` + `ROW name=…\|desc=…` + `END` |
| `CALL skills.get name=…` | `OK skills.get` + `LINE …` body lines + `END` |
| `CALL skills.save name=…` | writes under Application Support (body via `LINE`…`END` after request) |

Guest always knows **builtin** defaults even if the bridge is offline.

## Defaults

| Skill | Role |
|-------|------|
| `agent-plan-act` | Baseline plan/act loop |
| `capability-safe-tools` | Cap discipline |
| `email-triage` | Inbox via MCP email connector |
| `inbox-brief` | Short mail brief for UI/chat |
| `knowledge-search` | Curated corpus via `search.query` (tSearch-style) |

## Guest runner (v0.9.2)

The kernel does **not** interpret skill markdown. Named builtins map to a
fixed plan in `kernel/src/agent.rs` that issues MCP calls under the current
`Caps` and paints a Brief screen. Unknown / saved skills still use
`skills.get` for a body blurb only.

Also see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).
