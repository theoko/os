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
| `CALL skills.list` | `OK skills.list n=N` + `ROW name=…\|src=default\|saved\|desc=…` + `END` |
| `CALL skills.get name=…` | `OK skills.get` + `LINE …` body lines + `END` |
| `CALL skills.save name=… desc=… skills=1` | one-line starter under Application Support |
| `CALL skills.save name=… skills=1` then `LINE`…`END` | full body write |
| `CALL skills.forget` | Delete the user skills tree (defaults untouched) |

`skills.save` requires `skills=1` (guest `Cap::SkillsSave`). Without it the
bridge returns `ERR skills.save needs_skills_cap` and still drains any
`LINE`…`END` body so the protocol stays in sync.

Revoking Save skills on the guest calls `skills.forget` and refreshes the
Skills list — same consent loop as email / workspace / audio / portal.

Guest always knows **builtin** defaults even if the bridge is offline. With
Save skills granted, the Skills screen **Save starter** CTA and the
`capability-safe-tools` runner call `skills.save … skills=1` and refresh the
list so `src=saved` rows appear.

## Defaults

| Skill | Role |
|-------|------|
| `agent-plan-act` | Baseline plan/act loop |
| `capability-safe-tools` | Cap discipline |
| `email-triage` | Inbox via MCP email connector |
| `inbox-brief` | Short mail brief for UI/chat |
| `knowledge-search` | Curated corpus via `search.query` (tSearch-style) |
| `teddy-portals` | Teddy corpus API + live `teddy.*` portals |
| `market-portals` | Live `market.health` / `market.fear_greed` |

## Guest runner (v0.9.2+)

The kernel does **not** interpret skill markdown. Named builtins map to a
fixed plan in `kernel/src/agent.rs` that issues MCP calls under the current
`Caps` and paints a Brief screen. Unknown / saved skills still use
`skills.get` for a body blurb only until they gain a guest plan.

From **0.9.9** the guest also writes: `mcp::save_skill` sends the one-line
`desc=` form with `skills=1` when `Cap::SkillsSave` is on (cap refusal never
opens COM2).

From **0.9.11** every Skills row opens Brief — builtins run their MCP plan;
saved/unknown skills show a playbook body peek (`skills.get`) instead of a
footer blurb.

From **0.9.16** that peek is a **plan preview**: the guest scans the body for
known tool spellings, lists them as `Tool` lines, and marks `Need` for grants
still off.

From **0.9.20** the guest then **CALLs granted peek tools** named in the body
(`email.search`, `search.query`, `teddy.health`, `market.health`). Cap denial
never opens COM2. Path/URL/write tools (`audio.transcribe`, `doc.read`,
`skills.save`, `workspace.index`, `tsearch.sync`) stay Info-only — markdown is
still not a script, and `email.send` stays disabled. One-line `skills.save`
starters ship with suggested tool names so the plan is non-empty.

From **0.9.21** `calendar.list` is also a granted peek (same `email=1` /
`Cap::EmailSearch` as mail). The bridge refuses without the wire bit — no
ambient calendar just because LIST names the tool.

Also see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).
