---
title: The thesis after the substrate move
version: v01
date: 2026-07-26
status: historical decision record — not Live product docs for this tip
---

# The thesis after the substrate move

> **Tip note (teddyOS / `fix/bridge-prewarm`).** This essay settled the
> enforcement question *before* the standalone Linux live tip. Citations of
> `host/bridge/src/main.rs`, COM2, and MCP connector north-star wording describe
> the **pre-standalone / MCP-era tree** (or MCP `main`), not what this tip ships
> today. Current product truth: `AGENTS.md`, `STATUS.md`,
> `docs/architecture-os-doc-v01.md`. Do not treat path/line references here as
> Live on this fork.

`linux-os-doc-v02.md` ends with a sequence whose first step is *"settle the
enforcement question. Everything else follows from it."* Nothing follows until
this document is agreed. This is that step.

The owner has decided to move to a Linux substrate. That decision is not
re-argued here. What it does is retire a claim the project has been making, and
the retirement has to be written down before any code moves, because the claim
is baked into `AGENTS.md`, into `docs/architecture-os-doc-v01.md`, into the
searchable corpus that ships inside the OS, and into a doc comment at the top of
`kernel/src/caps.rs`.

---

## 1. The old claim, stated fairly

`AGENTS.md:34`, non-negotiable #3, in full:

> **Capability model.** Connectors are tools behind caps — no ambient root.
> `email.send` stays disabled until explicit confirm/cap policy.

`docs/architecture-os-doc-v01.md:39-43`, under the heading *Capability model
(non-negotiable)*:

> 1. No ambient root for agents or tools.
> 2. Holding a capability is the only way to exercise an authority (open this
>    directory, write this serial port, talk to that service endpoint).
> 3. The kernel checks the capability token on every relevant syscall/IPC.

And `kernel/src/caps.rs:5`, the module doc:

> No ambient root: a missing grant is a hard deny, not a soft skip-with-try.

Taken together the claim is: **authority in this system is held, not
ambient — a component can only do what it has been explicitly granted, and the
kernel is what makes that true.**

It is worth saying that this claim was true, and true for a good reason. It held
because the kernel is `no_std` and has no filesystem, no allocator, no ELF
loader, no syscall table, and exactly one path off the machine — a 16550 UART
into `mcp.rs`. There is nothing to escape *through*. That is enforcement by
construction, and it is the strongest form of the property: not a policy that
could be misconfigured, but an absence of mechanism.

## 2. Why real applications break it, precisely

The break is not subtle and it is not fixable by wording.

**2.1 — The kernel stops being the enforcement point and becomes a dependency.**
Point 3 above — *"the kernel checks the capability token on every relevant
syscall/IPC"* — describes a syscall surface we wrote, with a capability token we
invented, that does not exist and will never exist on Linux. Linux has ~400
syscalls it will happily serve to any process, and it checks DAC, LSM hooks, and
namespaces, not a `Caps` bitset. The instant `caps.rs` compiles as a userspace
library instead of a kernel module, point 3 is false. Not weakened — false. The
sentence describes a mechanism that will not be present.

**2.2 — A process that can `open()` is ambient by definition.** LibreOffice is
the concrete case and it is not a strawman: it reads `~/.config`, walks
fontconfig caches, writes lock files next to the document, spawns
`xdg-open`, and reads any path it is handed. It is 10 million lines nobody in
this repo will audit. Running it under a permission screen that says
"Your files: Search project folders you choose" while the process holds an
unrestricted `open()` produces a UI that describes a restriction nothing
applies. That is worse than having no permission screen, because it is a
truth claim to the user that is false.

**2.3 — The property was never portable; it was a side effect of poverty.**
This is the unsparing part. The project did not build a capability enforcement
mechanism. It built a machine with no mechanism to abuse, and the capability
model rode on top as bookkeeping. `Caps` is a `u8` bitset with seven bits
(`kernel/src/caps.rs:93-97`). It is checked by `mcp.rs` before it opens COM2 —
by the same binary that holds it, at a call site that could simply not call it.
The enforcement was "there is no other way to do anything," and the bitset was
the *record* of intent, not the barrier. Delete the poverty and the bitset is
a comment.

**2.4 — It is already breached in the code, today, before any migration.**
`host/bridge/src/main.rs:574`:

```rust
"email.send" if !matches!(arg_val(args, "email"), Some("1")) => {
    vec!["ERR email.send needs_email_cap".into()]
}
```

The bridge decides whether the caller holds the email capability by reading a
flag **the caller put in its own request line**. There is no proof of grant, no
peer identity, no token — the bridge takes the client's word for it.

To be precise about what this does *not* mean today: `email.send` cannot
actually send anything. Its handler is mock-only —
`host/bridge/src/main.rs:588` reads *"Mock only — never talks to gog"* and
returns `OK email.send mock queued`. An earlier draft of this document claimed
`CALL email.send to=… email=1 confirm=1` sends mail; that was wrong, and a
security argument that overstates its own evidence is worth less than no
argument. The defect is the *authorisation shape*, not a live exfiltration
path: the same self-asserted flag gates `email.search`, which does read real
mail when the Gmail backend is connected. That is survivable today for exactly
one reason: the only client is our own kernel, which is the enforcement point,
so a request that lies would have to be our own code lying. It is a *guest-side*
capability check with a *host-side* effect.

The bridge also listens on `127.0.0.1:7420` (`main.rs:56`) with no
authentication. Under the current architecture that port is reachable only from
a VM serial pipe. On a Linux desktop, `127.0.0.1:7420` is reachable by every
process on the machine. Every application. Including LibreOffice, and including
anything it opens. The email capability becomes ambient authority for the whole
system the day the substrate changes, and nothing in the UI would say so.

So the honest summary: the old claim depended on there being exactly one program
in the world, and we wrote it.

## 3. The new claim

> **A desktop where every application's access is explicit, and the agent is one
> of those applications.**

This is the reframe `linux-os-doc-v02.md:130-134` proposed, and it is the right
one. Three things to notice about it:

**It is a stronger product claim, not a weaker one.** "The agent asks before
touching your files" is a feature of one program. "Nothing on this machine
touches your files without appearing on this screen" is an operating system.
The second is the thing people actually want on a machine that also runs Word.

**It generalises what already exists rather than replacing it.**
`workspace.index`, `audio.transcribe`, `portal.sync` (`caps.rs:15-21`) are
already written as *user-facing consequences* rather than API surface — "Search
project folders you choose", "a recording can contain anyone, not just the
user". That copy is correct for any application, not just the agent. The
capability set is already the right shape; it applies to one process today.

**It moves the agent from privileged to peer.** Under the old claim the agent
was the subject of the permission system. Under the new one it is a tenant of
it, sandboxed like everything else, and its own access shows up in the same list
as LibreOffice's. That is a demotion the project should welcome: a permission
manager that exempts its own author's program is not a permission manager.

**What the new claim does not say.** It does not say the system is secure
against a determined attacker, and it must never be read that way. It says
access is *explicit and legible*. Those are different properties and section 4
is about the gap between them.

## 4. What actually enforces it on Linux

Four mechanisms, none sufficient alone, none a security boundary against a
kernel exploit. Taken together they are what the industry actually ships, and
that is the honest ceiling.

### 4.1 Namespaces — control what a process can *name*

Mount, network, PID, IPC, UTS, user, cgroup, time.

*Holds:* the mount namespace is the real filesystem control. A process cannot
`open()` a path that is not in its mount tree — this is not a filter that has to
enumerate the bad paths, it is an absence, which is the same shape of guarantee
the old kernel had and the only place we get it back. A network namespace with
no interface means no network, full stop, regardless of what the process
attempts.

*Does not hold:* namespaces are constructed by userspace and the construction is
where the bugs are — a leaked file descriptor across the boundary stays fully
usable, because namespaces restrict *name resolution*, not existing handles.
Unprivileged setup depends on user namespaces, whose availability is a distro
policy question (Ubuntu restricts them via AppArmor; others gate them on a
sysctl), so "we sandbox with bwrap" has a per-distro asterisk. And user
namespaces are historically the single richest source of local privilege
escalation CVEs in the kernel — the mechanism that makes unprivileged
sandboxing possible also enlarges the attack surface it is defending.

### 4.2 seccomp-bpf — control which syscalls exist

*Holds:* it shrinks the kernel attack surface, which is the mitigation for 4.1's
last paragraph. A filter that denies `keyctl`, `bpf`, `userfaultfd`, `ptrace`,
and the exotic socket families removes the exploit primitives most escapes are
built from. This is defence in depth and it is worth doing.

*Does not hold — and this is the part that gets overclaimed:* a seccomp filter
sees the syscall number and the six arguments **as scalars**. It cannot
dereference a pointer. Deliberately: the memory could be changed by another
thread between the check and the kernel's own read (TOCTOU), so the API refuses
to let you try. **Therefore seccomp cannot filter by path, by filename, or by
any string.** It cannot express "may open ~/Documents". Anyone who describes
seccomp as the thing enforcing a file-access permission is wrong. There is
`SECCOMP_RET_USER_NOTIF`, which punts the decision to a supervisor process that
can inspect the target's memory and inject file descriptors
(`SECCOMP_IOCTL_NOTIF_ADDFD`), and it does make path decisions possible — but it
is intricate, it has its own TOCTOU footguns, it serialises syscalls through a
userspace round trip, and it is not what we should be betting the file
permission on.

### 4.3 Landlock — unprivileged, path-based, inherited, irrevocable

The closest thing Linux has to what this project meant.

*Holds:* an unprivileged process can restrict itself to a set of filesystem
hierarchies with specific rights; restrictions are inherited by children and can
never be relaxed, only narrowed further. That "monotonically shrinking, no way
back" property is exactly capability discipline, and it needs no root, no
container, no daemon. It is the right mechanism for the agent to restrict itself
*and every tool it spawns* before it does anything interesting.

*Does not hold:* Landlock is versioned and its coverage grows per kernel — the
first ABI covered filesystem paths only; network arrived later and covers **TCP
bind and connect only** (no UDP, no per-host, no DNS, no unix sockets in the
early ABIs). The practical consequence is that the ruleset you can install
depends on the kernel you booted, so the code must probe the supported ABI at
runtime (`landlock_create_ruleset` with `LANDLOCK_CREATE_RULESET_VERSION`) and
decide, per feature, whether to degrade or refuse. **A "best effort" degrade is
a silent downgrade of the claim on the user's screen, so the Capabilities screen
must be able to say "this kernel cannot enforce this" rather than showing a
switch that does nothing.** That is a UI requirement falling directly out of a
kernel API detail, and it is the kind of thing that gets discovered late.

Also: Landlock is checked at path resolution. An already-open descriptor is
unaffected. Revoking a grant does not close what was opened under it, which
means revocation in the UI needs a defined meaning ("no new access" vs "access
ends now") and the honest one is the first.

### 4.4 Portals — where the grant actually comes from

`xdg-desktop-portal` + the document portal, as used by Flatpak, is the piece
that makes 4.1–4.3 usable rather than merely restrictive.

The file chooser runs **outside** the sandbox, in a process the application
cannot influence. The user picks a file. The portal hands the application a
descriptor to that one file through a FUSE-backed document store. The
application never had, and never gets, the ability to enumerate the tree.

This is the mechanism the project has been describing in its UI all along, and
it already exists. The user's act of choosing *is* the grant. PipeWire does the
same for camera, microphone, and screen capture. This should be the default path
for `workspace.index` and `audio.transcribe`, in preference to handing a sandbox
a whole directory.

*Does not hold:* portal grants persist once given and the revocation UI is
generally poor — this is where the project can add real value rather than
reimplement. And portals are opt-in: an application that never calls one and was
handed `--filesystem=host` simply reads everything.

### 4.5 What none of it holds — say this out loud

- **The kernel is the boundary, and it is enormous.** Every one of these is
  enforced by Linux. A kernel LPE voids all four simultaneously. The old claim's
  TCB was the whole no_std kernel — 18865 lines of `kernel/src`, about
  12,900 excluding tests — and the new one is Linux's. That is a real, permanent
  regression in the *strength* of the guarantee, traded for the ability to run
  software. State the trade; do not pretend it is not one.
- **A permission model is only as good as the permissions accepted.** In
  practice most Flatpak applications ship broad static permissions —
  `--filesystem=host` makes filesystem confinement decorative;
  `--socket=x11` means the app can keylog and screen-scrape every other X11
  client, because X11 has no client isolation and no sandbox can add it;
  `--talk-name=org.freedesktop.Flatpak` is a documented, intentional escape
  hatch that lets a sandboxed app run arbitrary commands on the host. **If this
  project ships a Capabilities screen and then installs applications with those
  permissions, the screen is a lie with extra steps.** Wayland, not X11, is a
  prerequisite for input and display isolation, and that belongs in the
  non-negotiables.
- **D-Bus and setuid binaries are the lateral path.** Confinement that stops at
  `open()` and lets the process talk to a session-bus service holding full
  access has moved the authority, not removed it. Every bus name granted is a
  capability and must appear as one.
- **Enforcement bounds what a program *can* touch, not what it *decides* to
  do.** For the agent specifically: an LLM that has been granted the inbox and
  reads an email containing instructions is a confused deputy operating entirely
  within its granted authority. No sandbox prevents this. Capability enforcement
  is a **blast-radius control, not an alignment control**, and the project must
  not let a strong sandboxing story imply the injection problem is solved. This
  is why the `email.send` confirm step is a genuinely separate defence and
  should survive untouched.

### 4.6 The one architectural decision this forces now

`linux-os-doc-v02.md:270` lists "bridge over vsock or a unix socket" as a step-2
implementation detail. It is not a detail; it is the enforcement design, and
section 2.4 is why.

A bridge on `127.0.0.1:7420` is reachable by every process in the network
namespace, so it is ambient authority by construction. A **unix socket file
descriptor passed into the sandbox at spawn time** is the opposite: it is
unforgeable, it is the only route out of a netns-isolated sandbox, and
possession of it *is* the capability. That is the actual capability model,
finally implemented by something rather than recorded by a bitset.

Two consequences:

1. **The capability check moves host-side.** The bridge must derive the caller's
   grants from *which socket the request arrived on* (and `SO_PEERCRED` /
   `pidfd` to bind it to an application identity), never from a flag in the
   request line. `main.rs:574` inverts today; it must be inverted back. This is
   the single highest-value change in the whole migration and it is
   approximately a day of work.
2. **`caps.rs` changes role, not code.** The bitset stops being the enforcement
   record and becomes the *policy* the host consults and the UI renders. Its
   tests — particularly `a_switch_grants_the_capability_it_names` at
   `caps.rs:257` — get more important, not less, because a consent screen that
   grants something other than what it names is now a system-wide failure rather
   than an agent-scoped one.

The bridge already supports `OS_MCP_BRIDGE_ADDR=unix:<path>`
(`host/bridge/src/main.rs:91-95`, `serve_unix` at `:199`). The foundation is
there; what is missing is per-peer grant derivation.

## 5. What the Capabilities screen becomes

It stops being a consent flow for one program and becomes **the system's
permission manager** — the place where the answer to "what on this machine can
see my files" lives, for every application.

What changes:

| | today | after |
|---|---|---|
| Subject | the agent | every installed application, agent included |
| Rows | 7 fixed caps (`Cap::ALL`) | per-app grant set over a shared vocabulary |
| Grant source | a switch at setup | a switch **and** a portal choice (picking a file is a grant) |
| Backing | `Caps` bitset the agent checks on itself | landlock ruleset + namespace + socket fd at spawn |
| Enforcement point | inside the process being constrained | outside it |
| Failure mode | agent skips a call | `open()` returns `EACCES` |

What must not change: the plain-language copy. `Cap::label` and `Cap::detail`
(`caps.rs:42-65`) are written for a person deciding what their machine may
touch, and the comments explaining *why* each default is off — "a personal file
tree is not something to opt someone into silently", "a recording can contain
anyone, not just the user" — are the most valuable prose in this repo. That
judgement is the differentiated asset. Flatpak and bubblewrap already do the
containment; nobody has built a legible consent surface on top of it. That gap
is the product.

Three new requirements the generalisation creates:

1. **Per-application scope.** "Your files" is no longer one bit; it is one bit
   per app, and the screen has to make "which app" as legible as "which
   capability" without becoming a matrix nobody reads.
2. **Honest unenforceable states.** Per 4.3, some grants cannot be enforced on
   some kernels, and some applications (X11, `--filesystem=host`) cannot be
   confined at all. The screen needs a third state beyond on/off —
   *unenforceable here* — and the project should refuse to show a switch it
   cannot back. This is the whole thesis in one UI decision.
3. **Revocation with a defined meaning.** Per 4.3, revoking stops new access and
   does not retract open descriptors. Say so, or restart the app.

## 6. Claims that are now false and must be edited

Not edited here — this document only identifies them.

### 6.0 There is no `README.md`

The task named `README.md` and `AGENTS.md` as the two places carrying the claim.
**`README.md` does not exist** — not in the working tree and not in
`git ls-files`. The repo's identity card is `AGENTS.md`, and `CLAUDE.md` is a
**stale near-duplicate of it**: same headings, same non-negotiables, but
`CLAUDE.md:9-11` still says x86_64-only and UTM-only, which `AGENTS.md:9-12`
already corrected to include ARM64 and VirtualBox. So the claim lives in two
files that have already drifted, and any edit must be applied to both or the
duplication should be collapsed. Line numbers below are given for both.

### 6.1 The central claim

**`AGENTS.md:34`** (identical text at **`CLAUDE.md:33`**):

> 3. **Capability model.** Connectors are tools behind caps — no ambient root.
>    `email.send` stays disabled until explicit confirm/cap policy.

- *"no ambient root"* — false the moment an unmodified binary runs. Must be
  replaced with what is actually enforced (section 4) or deleted. Section 2.4
  shows it is arguably already false.
- *"Connectors are tools behind caps"* — not false, but it scopes the model to
  connectors, which is the old thesis. The new claim covers applications.
- *"`email.send` stays disabled until explicit confirm/cap policy"* — **keep
  this.** It is enforced host-side at `main.rs:577` with a test at `:1370`, it
  survives the migration untouched, and per 4.5 it is a separate and necessary
  defence.

### 6.2 The north star

**`AGENTS.md:26-27`** (**`CLAUDE.md:25-26`**):

> North star: **capability-based agents**. MCP connectors (email, search, …) run
> on the **host bridge**, not in the kernel. Guest holds caps and calls tools
> over COM2 until a guest network stack exists.

- *"capability-based agents"* — the noun is wrong. It becomes capability-based
  *applications*, of which the agent is one.
- *"Guest holds caps"* — this is precisely the inversion section 4.6 kills. The
  guest holding its own caps is the bug. The host must hold them.
- *"over COM2"* / *"until a guest network stack exists"* — obsolete; the
  transport becomes a passed unix socket fd, and the network stack now exists.

### 6.3 Substrate description

**`AGENTS.md:9-12`** (**`CLAUDE.md:9-11`**, additionally stale):

> Agent-centric hobby operating system in Rust (`no_std` kernel), targeting
> **x86_64 and ARM64**, booted with **Limine**. Headless x86 runs use **QEMU**;
> desktop runs use **UTM** for x86 emulation or **VirtualBox** for native ARM64
> virtualization on Apple Silicon.

`no_std` kernel, Limine, UTM, and the whole x86 emulation story go. Per
`substrate-measurement-os-doc-v01.md`, aarch64 + HVF + virtio-gpu is the
target and it is not optional.

**`AGENTS.md:17`**:

> `kernel/          freestanding Rust kernel (Limine + framebuffer UI + COM2 MCP client)`

All three parenthesised items are deleted by the move.

### 6.4 Non-negotiables 1 and 4

**`AGENTS.md:31`** (**`CLAUDE.md:30`**):

> 1. **QEMU first for CI.** `make test` = host unit tests + QEMU serial smoke +
>    MCP bridge smoke.

Not a thesis claim, but it goes false with the x86 serial smoke. The replacement
must keep a real gate — this is the line that has been keeping the repo honest
and it should not quietly become "host unit tests" only.

**`AGENTS.md:35`** (**`CLAUDE.md:34`**):

> 4. **Inference and HTTP stay out of the kernel.** Bridge + future userspace
>    only. Skills are markdown playbooks, not privileged code.

Survives in spirit, but *"the kernel"* now means Linux, and nobody was proposing
to put HTTP in Linux. Reword to the intended property: inference and network
egress stay outside the trusted computing base and behind a capability. The
second sentence — skills are data, not code — is a genuine security property,
becomes more important when the sandbox is doing the work, and should be
promoted rather than left as a trailing clause.

### 6.5 Beyond the two files named — the claim is in three more places

**`docs/architecture-os-doc-v01.md:39-43`**, still marked `status: active`,
states the capability model as *non-negotiable* and asserts:

> 3. The kernel checks the capability token on every relevant syscall/IPC.

Flatly false on Linux (section 2.1). The doc also specifies `mint_cap` /
`grant` / `drop_cap` syscalls and a five-phase plan ending in "`agentd` +
sandboxed tools" that the move deletes. It needs `status: superseded` and a v02,
not a patch.

**`search/corpus.json:30`** — the claim ships *inside the OS* as a searchable
answer:

> "Kernel owns scheduling memory capability tables message bus. Userspace owns
> agentd sandboxed tools. No ambient root. Syscalls send recv grant mint_cap."

This is the worst instance. A user typing "capability" into the built-in search
gets told the system enforces something it does not. `search.query` is the one
capability granted by default (`caps.rs:102-106`), so this is reachable on a
first boot with nothing else enabled. It must be corrected in the same change as
`AGENTS.md`, and a corpus re-bake (`./scripts/bake-corpus.py`).

**`kernel/src/caps.rs:5`**:

> No ambient root: a missing grant is a hard deny, not a soft skip-with-try.

Flagged, not edited: `kernel/src/` is owned by another session. The *behavioural*
half ("hard deny, not a soft skip") stays true and is worth keeping; the
"no ambient root" framing does not survive and the sentence needs whoever owns
that tree to rewrite it.

Lower priority, same vocabulary: **`docs/ui-os-doc-v01.md:48`** — "Same —
Advanced is not ambient root" — remains true as a statement about defaults, but
inherits a retired term and should be rephrased when the table is next touched.

## 7. What this settles, and what it does not

Settled: **option 2 from `linux-os-doc-v02.md:229-242` — real sandboxing.** Options
1 and 3 assume the only thing running is code we wrote, which the decision to
run real applications has removed from the table.

Also settled, and new here: the capability check moves host-side and the
transport becomes a passed socket descriptor (4.6). That is the difference
between a capability model and a capability *diagram*, and it is cheap enough
that there is no reason to sequence it late.

Not settled, and needing decisions before step 2 of the migration sequence:

- **bubblewrap directly, or Flatpak's full stack?** bwrap is the mechanism with
  no policy and no app store; Flatpak brings portals, a permission vocabulary,
  and a distribution story, along with the broad-permission problem in 4.5.
- **What happens to an application that cannot be confined?** Refuse to run it,
  run it and mark it unconfined in the UI, or fall back to a VM. The answer
  determines whether the Capabilities screen is describing or deciding.
- **Wayland-only?** Section 4.5 says it has to be. That should become a
  non-negotiable rather than a preference, because X11 silently voids the
  display half of every grant on the screen.

---

*The claim gets weaker in one dimension — the trusted computing base goes from
the ~18865 lines of kernel we wrote to all of Linux — and stronger in every other. The old
claim was true about a machine that could not run anything. The new one is a
claim about a machine people would use, and it has to be defended by mechanism
rather than by absence. Write it down before writing code, because the current
wording will otherwise ship into a system that contradicts it.*
