---
project: teddyOS / os
type: skills
purpose: agent skill playbooks on the standalone tip
status: active — this tip; bridge skills API removed
---

# Skills (v01) — standalone tip

> **Tip note.** Skills here are **markdown playbooks**, not a live host
> registry. There is **no** `CALL skills.list` / `skills.get` / `skills.save`
> bridge on this tip. MCP `main` still has that connector story; this fork does
> not. See `STATUS.md` and `AGENTS.md`.

## What a skill is

Cursor-style `SKILL.md` packages the agent (or a human) reads before acting.
They are **not** kernel code and **not** privileged.

## Layout (this tip)

```
skills/defaults/<name>/SKILL.md     # shipped with the OS repo
```

Frontmatter: `name`, `description` (same convention as Cursor skills).

There is **no** Application Support save tree wired through a host bridge on
this tip. Do not document `~/Library/Application Support/os/skills/` as Live
behavior here.

## Defaults (shipped)

| Skill | Role on this tip |
|-------|------------------|
| `agent-plan-act` | Baseline plan/act loop |
| `capability-safe-tools` | Cap discipline |
| `email-triage` | Inbox playbook — connectors Offline until network |
| `inbox-brief` | Short mail brief copy |
| `knowledge-search` | Curated **baked** corpus (see search doc) |
| `teddy-portals` | Historical / aspirational portal tools — Offline |
| `market-portals` | Historical / aspirational market tools — Offline |

Playbooks that still spell `CALL email.search` / `search.query` / `teddy.*`
are **instructions for agents**, not proof that a host MCP process answers.
On this tip, freestanding search answers from the baked corpus; mail and live
HTTPS stay Offline stubs.

## Guest behavior

The kernel does **not** interpret skill markdown as a script. Named builtins
may map to a fixed guest plan; unknown skills are documentation only.

Cap denial and Offline stubs never invent host state. `email.send` stays
disabled without explicit confirm/cap policy on products that still wire it;
this standalone tip does not restore a send path via docs alone.

Also see [`docs/search-os-doc-v01.md`](search-os-doc-v01.md).
