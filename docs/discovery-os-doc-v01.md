---
title: Customer discovery kit — the two weeks before any more code
version: v01
date: 2026-07-27
status: current — supersedes feature work until 15 interviews are logged
---

# Customer discovery kit

On 2026-07-27 the owner took a strategy call with Mohamed Elayouty. The verdict, in the
owner's own framing, was *idea-obsessed, not problem-obsessed*. The agreed direction is to
stop adding surface to this project and go find out who has the problem. This document is
the instrument for doing that. It is deliberately operational: a niche test, a script, an
outreach message, a log schema, and a cut list. Nothing here is a plan to build anything.

The exit condition is stated up front so it can be checked rather than felt:

> **15 logged interviews in one sector, of which at least 5 describe a workaround they
> already pay for in money or hours.** Below that number, no feature work resumes.

---

## 1. Choosing the sector — and why the hardware wall picks it for you

The call surfaced the adoption problem honestly: local inference needs an Nvidia card and a
tech-savvy setup, and Mohamed's own Open Claw experience is evidence that local-first is a
much harder sell than cloud. That is correct — *for anyone who could have used a cloud tool
instead.*

So invert it. Do not look for people who would *prefer* local. Look for people for whom
cloud is **prohibited**:

- buy-side / independent investment research operating under MNPI handling and retention rules
- law firms whose client engagement terms forbid third-party processing of matter documents
- anyone bound by a DPA, BAA, or ITAR clause that names third-party LLMs explicitly

For that group the hardware requirement is not a tax. They already own the box, their
security team already refused the SaaS tool, and "it never leaves the machine" is the only
sentence in the pitch that matters. The constraint that looks like a weakness in a general
market is the qualifying filter in this one.

Note this rules out inheriting Mohamed's own wedge. Construction is a real market and he
knows it well, but a general contractor has no rule forbidding ChatGPT, so the local-first
story buys nothing there and the product competes on features it does not have.

**Recommended sector for the first 15: investment research.** Second choice: small law.
Pick one and do not sample both — the point of a narrow niche is that the fifteenth
conversation sounds like the first.

---

## 2. The interview script

Rules that matter more than the questions:

1. **Do not demo.** Not at the start, not in the middle. Once they have seen the thing, every
   later answer is about the thing instead of about their week. If they ask, say you will
   show them after and that you would rather hear how they work first.
2. **Ask about the last time, never about hypotheticals.** "Would you use…" produces
   politeness. "Walk me through the last one" produces facts.
3. **Ask for the workaround, then for the receipt.** Enthusiasm is free; a workaround costs
   something, and that cost is the only evidence of real pain.
4. **Shut up.** The useful part is usually after the pause you were tempted to fill.

### Warm-up (2 min)

1. What's your role, and what does a research week actually look like for you?

### The task, reconstructed (10 min)

2. Walk me through the last memo/report/deep-dive you produced. Start from the moment you
   were asked for it.
3. Where did the inputs live? Name the actual places — inbox, shared drive, a folder on your
   laptop, a data terminal, someone's head.
4. What did you already have and couldn't find? How long did re-finding it take?
5. What did you end up re-doing because you couldn't find the earlier version of it?
6. Who else had to be involved, and what did you have to wait on them for?

### The rules (5 min) — this is the qualifying section

7. What are you not allowed to put into an outside AI tool? Who told you that — policy,
   compliance, a client contract, or your own judgment?
8. What do you do instead, in the cases where you would have used one?
9. Has anyone in your firm been told to stop using a tool they liked? What happened?

### The workaround and its price (8 min)

10. What have you cobbled together to deal with this? Show me if you can.
11. What does that cost — a subscription, an analyst's hours, a contractor, your evenings?
12. If it disappeared tomorrow, what breaks?

### Close (2 min)

13. Who else does this job the way you do? Would you introduce me to two of them?

**The one question that decides everything: #10–11.** If they have built nothing, bought
nothing, and lost no hours, the pain is not real regardless of how much they nodded. Log it
as a no and move on — a clean no is worth as much as a yes and costs a lot less to collect.

---

## 3. Outreach

Short, specific, asks for 20 minutes, promises nothing, sells nothing. Send ~40 to land 15.

> **Subject:** how you handle research files — 20 min?
>
> Hi {name} — I'm Theo, based in Boston. I'm researching how investment teams handle their
> own documents and notes when compliance rules keep them off the usual AI tools.
>
> I'm not selling anything and there's nothing to demo. I'm trying to understand how the work
> actually gets done today — where the files live, what gets re-found, what the rules force
> you to do by hand.
>
> Would you have 20 minutes in the next two weeks? Happy to share what I learn across the
> other conversations, which is usually the useful part.
>
> Theo

Channels, best first:

- **The Greek network.** It already produced the Boston construction-AI founder. Warm intros
  convert at an order of magnitude above cold, and this is the largest warm surface available.
- **BC alumni** in finance in Boston — same argument, and the alumni frame makes a 20-minute
  ask cheap to say yes to.
- **Existing teddysearch subscribers**, who already opted into something adjacent and are
  therefore pre-qualified as interested in the problem space.
- Cold LinkedIn to compliance officers and heads of research at sub-50-person funds, last.

### 3.1 What the mailbox actually contains (checked 2026-07-27)

The three Gmail accounts were searched for finance correspondents. The honest result: **there
is no list of investment-research contacts sitting in the inbox.** The finance mail is
newsletters — CNBC Pro, myFT, a16z, Quiver, Unusual Whales, Burry's Substack. Consuming that
much market content is not the same as knowing people who do the job, and building the target
list will be real work rather than an export.

Three genuine assets did surface, and they are worth more than forty cold names:

**1. EFG Bank — George Georgiadis and Maxime Lejeune (relationship managers, live two-way
thread, account 434388).** This is the strongest lead in the mailbox and it was hiding in
plain sight. Private banking is the target profile almost exactly: client financial data,
hard confidentiality rules, an explicit prohibition on third-party processing, and a research
and reporting workload done by hand because of it. They are also Greek-network warm.
The ask, kept entirely separate from the account thread:

> George — unrelated to the account, and no rush. I'm doing research on how people in private
> banking handle client documents and notes given the rules about outside AI tools. Not
> selling anything. Would you have 20 minutes sometime in the next two weeks, or is there
> someone on the research side you'd point me to?

**2. Boston College Carroll — the part-time MBA cohort.** This is the best sample frame
available and it is already paid for. A part-time MBA is by definition working professionals,
and this is Boston: Fidelity, Wellington, MFS, Putnam, State Street, plus a long tail of
sub-50-person funds. Classmates will take a 20-minute call from a classmate at a rate no cold
channel approaches. `cgsom.career@bc.edu` runs the career newsletter and maintains the alumni
database — asking them to point at finance alumni is a normal, expected request.

Caveat worth naming: there is an active Part-Time → Full-Time transfer request in flight with
David Zhao. If that changes cohorts, the classmate list changes with it.

**3. Stephen Levis (access-avidity.com)** — recruited for a "AI × Finance startup" backend
role in May. A recruiter working that niche has the map of who in Boston is building this and
who is buying it. Worth 20 minutes purely for the landscape, with no pitch at all.

Order of operations: EFG first (warmest, best profile), Carroll career office second (highest
volume), Levis third (map, not sample). Everything else is cold and comes after those are
exhausted.

---

## 4. The log

`docs/discovery-log.csv` — one row per conversation, filled in within an hour of the call
while it is still accurate. The columns exist to make patterns visible instead of remembered;
the memory of fifteen interviews is a story, the log is data.

The three columns that decide whether this project continues, and what each means:

- `cloud_prohibited` — is there an actual rule, not a preference? This is the sector filter.
- `workaround` — what have they already built or bought? Empty means no real pain.
- `pays_today` — money or hours, with a number. This is the only column that predicts revenue.

Review after 5, 10, and 15. At each checkpoint write one sentence: *the problem I keep
hearing is ___.* If that sentence changes between checkpoints, the sector is wrong or the
script is leading.

---

## 5. Simplification — what the call actually implies for this repo

Mohamed's feedback that "search reinvented" means nothing to an outsider generalises past the
tagline. The same failure is live in shipped copy right now —
`~/Desktop/projects/droplet/new/tsearch-revival/index.html` describes the product as
"crawler, hybrid tf-idf + LSA ranking, extractive AI overviews with citations, and a
knowledge graph." Four technical nouns and no outcome; nothing in it tells a stranger what
changes for them.

Candidate replacements, all of which are testable against the interviews rather than settled
here:

- "Ask questions about your own files. Nothing leaves your machine."
- "The research assistant your compliance team will actually approve."
- "Your documents, searchable, on hardware you own."

**The harder question the call raises, which discovery should settle:** whether this is an
*operating system* at all. Nobody buys an OS; they buy one task getting done. The Rust kernel
is real engineering, and `docs/thesis-os-doc-v01.md` has already retired the capability claim
that justified it and moved to a Linux substrate — so the strongest remaining reason to ship
an OS rather than an application is gone. If fifteen interviews describe a document problem,
the sellable form of this work is an app on a machine they already own. That is a product
call to make *after* the interviews, but it should be made explicitly and written down here
as v02, not drifted into.

Deferred until then, and not to be worked on in the meantime: any new lane, any new connector,
any new UI surface. The known-open items in `docs/` (the Search-lane offline-openability
contradiction, the stopword ranking gap) stay open unless an interview names them.

---

## Checkpoints

- [ ] 40 outreach messages sent
- [ ] 5 interviews logged → write the one-sentence problem statement
- [ ] 10 interviews logged → restate it; note what changed
- [ ] 15 interviews logged → go / pivot / stop decision, written as `discovery-os-doc-v02.md`
- [ ] Follow-up call with Mohamed, mid-August 2026
