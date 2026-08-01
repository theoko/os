---
project: os
purpose: non-technical UX audit of the Linux daily-driver product
status: active
version: v01
date: 2026-07-29
---

# Non-technical UX audit — teddyOS Linux desktop

**Requirement:** every screen, error, and empty state must make sense to someone
who has never heard of git, SSH, packages, flags, “helpers,” or a terminal.
Engineers may get precision in logs — never as the only path.

This audit covers the **Linux live product** (`linux/`), not the freestanding
kernel. Audience: **clueless new user on first boot and first “work on …”**.

---

## Verdict

**The happy path meets the design requirement** for a non-technical new user:

```
Boot → Guided setup → Get Started
  → Search (magnifying glass on the bar)
    → “work on my-project”
      → IF not signed in:
           “Let’s get you ready…” → Continue
           → Getting you ready (one browser sign-in at a time)
           → return to Search with question restored
      → IF signed in:
           “What do you need help with?” → Get help
           → Answers (one window, cards — no black terminal)
```

A new user **never needs to invent** “open helpers,” open Terminal, type a
command, or discover an app called Connect. Remaining risk is **product
complexity** (many third-party AI accounts) and soft status probes for a few
vendors — not a broken first-run story.

**Overall grade: Pass (with watch items)**

---

## Scorecard

| Area | Grade | Notes |
|------|-------|--------|
| Dock always visible | **Pass** | `dock-fixed`; no Super-key dependency |
| Dock apps | **Pass** | Search, Web, WhatsApp, Files, Install (live only). No Terminal; Claude tile removed |
| First-boot setup | **Pass** | Calm intro, Guided default, honest capability badges |
| Welcome / Get Started | **Pass** | “Start working on something” → Search; dock explained |
| Work-on primary path | **Pass** | Continue-only until signed in; no vendor catalog first |
| Sign-in walkthrough | **Pass** | “Getting you ready”; Chromium; device codes prefilled |
| Return after sign-in | **Pass** | `pending-ask` restores project + prompt; auto re-open Search |
| Get help / Answers | **Pass** | One window; headless AIs; plain errors |
| Chat tools never open TTY | **Pass** | Search routes chat AIs to ask-all only |
| Grok false “ready” | **Pass** | Real probe; skipped until login |
| GitHub download | **Pass** | `gh repo clone`; plain clone errors + Sign in row |
| Copilot headless | **Pass** | Allow flags; friendly permission errors |
| Secondary tool list | **Pass** | Hidden until someone is ready |
| Vendor multi-sign-in | **Inherent** | Mitigated by sequential Continue |
| Gemini / Antigravity probes | **Watch** | Soft “May need sign-in” still possible |
| Power-user CLI still on disk | **OK** | Hidden from dock; reachable only if user already knows names |
| Web result badges | **Pass** | Web / Guide / Your files / Help |

---

## Journey map (intended)

```
Boot → Setup (Hello. You don’t need to know how any of this works.)
  → Experience (Guided) → Capabilities → Skills → Ready
  → Get Started → Start working on something → Search
    → type “work on <project>”
      → not ready: Continue → Getting you ready → browser(s)
           → Search reopens → toast “tap Get help”
      → ready: type need → Get help → Answers cards
```

Words a new user should **never** need: helper, CLI, OAuth, gh, SSH, agent,
flag, argv, stdin, VTE, Connect your helpers.

---

## Findings by severity

### Critical (fixed)

| ID | Finding | Status |
|----|---------|--------|
| C1 | Dock auto-hide → no way to switch apps | **Fixed** — always visible |
| C2 | Get help opened raw CLIs / Terminal | **Fixed** — ask-all only |
| C3 | Connect showed VTE for login | **Fixed** — browser + status UI |
| C4 | False “Connected” (Codex / Perplexity) | **Fixed** |
| C5 | Git clone HTTPS no password UI | **Fixed** — `gh repo clone` |
| C6 | Copilot “permission denied” | **Fixed** — allow flags |
| C7 | Grok ready while not signed in | **Fixed** |
| C8 | First screen listed vendors / “Connect helpers” | **Fixed** — Continue-only |
| C9 | Claude dock tile → black terminal window | **Fixed** — removed from favorites; desktop NoDisplay / routes to Search |
| C10 | After sign-in, user lost question | **Fixed** — pending-ask + auto Search |

### High (addressed this audit)

| ID | Finding | Fix |
|----|---------|-----|
| H1 | “Connect your helpers” window title & copy | **Getting you ready**; plain errors |
| H2 | Ask-all / clone / toast still said “helpers” | Softened to Search / sign-in language |
| H3 | “Or open one helper” on first-timer card | Calm single path until ready |
| H4 | Setup caps note said “index / 68 MB ranking” | Plain offline-download language |

### Medium (open / watch)

| ID | Finding | Recommendation |
|----|---------|----------------|
| M1 | Many AIs ⇒ many sign-ins | Prefer **Claude (+ GitHub)** first in setup queue for brand-new users; rest optional |
| M2 | Gemini / Antigravity soft status | Add real auth probes like Grok |
| M3 | `teddyos-claude` / agent binaries still on PATH | Keep NoDisplay; never re-pin to dock |
| M4 | Setup capabilities list is still long | Keep Guided; optional “recommended defaults” one-tap later |
| M5 | Live Install tile only | Already removed post-install via dock-install hook |
| M6 | No empty-state illustration in Search | Optional; current quiet line is enough |

### Low

| ID | Finding | Recommendation |
|----|---------|----------------|
| L1 | Journal CSS/theme warnings | Cosmetic |
| L2 | Advanced “Or pick one by name” after ready | OK — advanced path only |
| L3 | Perplexity web-only | Fine for non-tech |

---

## Surface checklist

### Dock
- [x] Always on screen  
- [x] Search, Web, messaging, Files — no Terminal, no bare AI CLI  
- [x] Install only while live  
- [x] Custom icons for Search / brands / Answers  

### First boot
- [x] Calm intro animation  
- [x] Guided default  
- [x] Capability copy for non-technical  
- [x] Get Started points at Search + dock  

### Search
- [x] Placeholder: work-oriented  
- [x] Work-on: Continue until ready, then Get help  
- [x] Clone errors plain + Sign in recovery  
- [x] Pending ask restored after setup  
- [x] Chat tools → Answers, never TTY  

### Getting you ready
- [x] Animation “Let’s set you up”  
- [x] Chromium for OAuth  
- [x] Device codes: copy + prefilled URL  
- [x] Queue: Continue through missing sign-ins  
- [x] Return to Search when queue ends (or already set up)  

### Answers
- [x] Single window, cards  
- [x] Copilot permissions  
- [x] Skip unsigned tools at Search layer  
- [x] Errors point back to Search / Continue, not a mythical helpers app  

### Honest limit
**Non-technical success still depends on third-party browser sign-in**
(Anthropic, xAI, Google, GitHub). teddyOS can only make that step sequential
and calm — it cannot remove vendor accounts. The design requirement is met when
the user never has to *discover* that step; **Continue** is the discovery.

---

## Manual test script (new user)

1. Fresh session, no AI logins (GitHub optional).  
2. Complete setup (Guided). Dismiss Get Started via “Start working…”.  
3. Dock: Search, Web, WhatsApp, Files — no Terminal, no Claude tile.  
4. Open Search → `work on <known repo or local folder>`.  
5. Expect: **only** “Let’s get you ready…” + **Continue** (no vendor list).  
6. Continue → Getting you ready → browser sign-ins.  
7. After queue: Search reopens; toast to Get help.  
8. Type a question → Get help → Answers cards only.  
9. Failures: plain language + Sign in — never a black terminal.  
10. Optional: clone a private GitHub repo without prior gh login → Sign in row works.

---

## Changelog of audit response

- **v01:** Full audit; pending-ask loop; first-timer UI; remove Claude from dock;
  kill user-facing “helpers” copy; plain setup/search/answers language.
