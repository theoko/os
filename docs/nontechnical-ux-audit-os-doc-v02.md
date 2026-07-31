---
project: os
purpose: complete non-technical UX audit of the Linux daily-driver product
status: active
version: v02
date: 2026-07-30
---

# Non-technical UX audit — teddyOS Linux desktop (v02)

**Requirement:** a clueless new user (no terminal, git, OAuth, packages,
“helpers,” SSH) can boot the machine and get real help — including AI-drafted
LinkedIn/WhatsApp replies — without inventing an app or a command.

**Scope:** `linux/` daily-driver product (setup, tour, dock, Search, Getting
you ready, Answers, messaging wrappers). Not the freestanding kernel.

**Method:** code + copy review of all primary surfaces; live guest check
(`192.168.64.2`); contract/e2e jargon guards; fix pass for P0 restore bug.

---

## Verdict

| | |
|--|--|
| **Overall** | **Conditional pass** — project “work on …” path is solid; messaging + AI-reply path is now *designed* for nontechnical users and **P0 restore is fixed**, but not a clean ship until soft-auth readiness and email skill honesty land. |
| **Grade** | **Pass with open P1/P2** (was fail on freeform restore before this audit’s fix pass) |

```
Boot → Guided setup → Get Started tour
  → Search (first dock icon)
    → “work on …”  OR  “reply to my LinkedIn/WhatsApp messages”
      → not signed in: Continue → browser sign-in → Search restores same job
      → signed in: Get help / Draft my replies → Answers (AI drafts; you send)
```

A new user **should not** need Terminal, “open helpers,” or CLI. Remaining
risk is multi-vendor OAuth fatigue, soft probes (Gemini/Antigravity), and
skills that overpromise email.

---

## Happy path (what they actually experience)

### First boot — **Pass**

- Fullscreen setup: “You don’t need to know how any of this works.”
- **Guided** default; capability rows use plain labels (`Built-in help`,
  `Your files`, `Search the web`) — not `portal.sync` unless Advanced.
- Skills pick list; corpus download as “Getting search ready…” (no
  `teddyos-search --sync` instruction).

### Dock — **Pass**

Live guest favorites:

`Search`, `Web`, `WhatsApp`, `Files`  
(no Terminal, no bare Claude tile)

Desktop `Name=` / `Comment=` jargon sweep: clean.

### Tour (Get Started) — **Pass** (improved this audit)

- Dock, Search, “work on …”, Web/WhatsApp/Files, control.
- Now also: AI reply for WhatsApp/LinkedIn in plain words
  (“AI drafts; you always send”).

### Search → project help — **Pass**

- Placeholder: “What do you want to work on?”
- Not signed in: single **Continue** card (no vendor soup first).
- Continue → **Getting you ready** → Chromium sign-in → return + toast.

### Search → LinkedIn / WhatsApp AI replies — **Pass with caveats**

| Step | Behavior |
|------|----------|
| Phrase | `reply to my linkedin messages` / `reply to my whatsapp messages` |
| Open | Messaging / WhatsApp Web app window |
| Copy | “AI will draft… nothing is sent without you” |
| WhatsApp | QR coaching: “If you see a square code, scan it with WhatsApp on your phone once.” |
| Button | **Draft my replies** (auto-start when AI ready) |
| After sign-in | **Restores original query + kind** (fixed in v02 — was P0) |

### Answers — **Pass**

One window, cards, headless AIs (no black terminal TUI from Search).

---

## Critical (P0) — status after this audit

| ID | Issue | Status |
|----|--------|--------|
| **P0-1** | Pending-ask restore rewrote freeform/messaging as `work on <home>` and could dump the AI playbook into the entry | **Fixed** — `pending_ask` stores `kind` + `query`; Search re-runs human words |
| **P0-2** | Soft auth (Gemini/Antigravity) never `ok=True` → Get help stuck on Continue if only those tools exist | **Open** — mitigated when Claude/Grok/etc. hard-probe; still a trap on soft-only images |

---

## High (P1)

1. **Email skills overpromise** — setup offers “Catching up on email” with no
   Gmail/inbox product path comparable to WhatsApp/LinkedIn.
2. **Sign-in marathon** — sequential Claude/Grok/Copilot/Gemini/Codex/… still
   possible; should default first-time queue to Claude (+ GitHub) only.
3. **Software Update desktop** — `Terminal=true` in app grid (if found).
4. **WhatsApp from dock** — opens chat only; AI co-pilot still needs Search
   phrase (tour now mentions it; idle hint too).
5. **LinkedIn not on dock** — discoverability via Search/tour only (OK if
   tour + idle hints stay).

---

## Medium (P2)

| Issue | Notes |
|-------|--------|
| Progress badges still say “helpers” in places | User-visible strip |
| “Command window” under advanced tool list | After ready only |
| Multi-AI Answers for a simple paste-reply | Prefer one co-pilot for messaging |
| Search blur-close when browser steals focus | Can feel like a crash |
| `linux/README.md` is engineer CLI-first | Repo docs only |

---

## Surface scorecard

| Surface | Without terminal? | Jargon | Dead ends |
|---------|-------------------|--------|-----------|
| Setup | Yes | Guided clean | Email skills overpromise |
| Welcome tour | Yes | Clean | Skip OK |
| Dock | Yes | Clean | App grid power tools |
| Search (keyword) | Yes | Clean | Thin corpus → web row |
| Search (work on) | Yes | Clean | Soft-auth readiness (P0-2) |
| Search (LI/WA AI) | Yes after fix | Cleaner | Discoverability |
| Getting you ready | Yes (browser) | Connect buttons residual | Multi-account fatigue |
| Answers | Yes | Clean | Depends on real auth |
| Software Update | **No** | Terminal | Black window |

---

## What’s working well

1. Nontechnical first-boot story and Guided capability copy.
2. Dock discipline (Search first; no Terminal).
3. Work-on Continue-only until ready.
4. Getting you ready (Chromium app sign-in, device codes).
5. Answers headless path; chat tools never open TTY from Search.
6. Messaging product model: open app + AI draft + you send.
7. Automated jargon checks on desktop files + tour structure.
8. **Pending-ask messaging restore (v02).**

---

## Fixes landed in this audit pass

1. `pending_ask.py` — `kind` + `query` fields; messaging never requires
   storing the long system playbook as the only restore key.
2. `teddyos-search-app` — save/restore freeform & LinkedIn/WhatsApp by
   re-running the original Search words; user-facing “AI helper” → “AI”;
   WhatsApp QR coaching; idle hints for reply phrases.
3. Welcome tour — AI reply for WhatsApp/LinkedIn called out.
4. Credit labels — “May need a one-time sign-in” (no “Connect if it fails”).
5. Soft account status — “Tap Continue to sign in.”
6. Contract tests for messaging pending-ask kinds + restore source.

---

## Recommended next work (impact order)

1. **P0-2** — real probes or post-Connect optimistic ready for Gemini/Antigravity.
2. **First-time sign-in queue = Claude only** (+ GitHub when cloning).
3. **Honest skills** — hide/reword email until inbox path exists (or ship it).
4. **Messaging auto-start prefers one AI** (Claude) for less scary Answers.
5. **Software Update without Terminal=true.**
6. **Strip remaining user-visible “helpers”** in progress badges.

---

## Consistency vs design docs

| Claim | Reality |
|-------|---------|
| Nontechnical first | Primary UI yes |
| Search is the product | Dock #1 + tour |
| AI helps reply; never auto-send | Yes |
| No CLI / OAuth / helpers on screen | Mostly; residual Connect/helpers |
| Pending restore after sign-in | **Yes for work + freeform/messaging (v02)** |
| Chat tools never open TTY | Yes from Search |

### Bottom line

The OS **mostly meets** the nontechnical requirement on the **project help**
spine and now **intends to** on **LinkedIn/WhatsApp AI replies**, with the
sign-in restore loop fixed. It is not yet a clean “clueless daily driver”
for every freeform task (email skills, soft-auth traps, multi-OAuth). Ship
messaging as a happy path only with P0-1 fix deployed (done) and P0-2
watched on images that lack Claude.
