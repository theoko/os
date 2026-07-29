"""Capability grants, and what actually enforces them.

`docs/thesis-os-doc-v01.md` §2.2 is the constraint this module is written
against: a permission screen that describes a restriction nothing applies is
worse than having no permission screen, because it makes a truth claim to the
user that is false. So this file is deliberately small, and it does NOT pretend
to be the enforcement point.

What is actually true on this substrate:

  portal.sync      ENFORCED. teddyos-search runs under a systemd unit whose
                   PrivateNetwork is toggled by the grant. Denied means the
                   process is in a network namespace with no interface — not a
                   flag it could decline to check. This is the same shape of
                   guarantee the no_std kernel had (an absence of mechanism),
                   and it is the only one here that deserves the word.

  workspace.index  ENFORCED for paths. The indexer runs with BindReadOnlyPaths
                   limited to the folders chosen on the setup screen, so a path
                   outside them cannot be named, let alone opened.

  search.query     NOT a restriction. The built-in corpus ships inside the
                   image and is readable by anything on the machine. The switch
                   exists to turn the feature off, not to contain it, and the
                   setup screen says so in those words.

Anything added here later must come with an answer to "what stops a process
that ignores this file", and if the answer is "nothing", the UI has to say
that rather than imply otherwise.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

SYSTEM_CONF = Path("/etc/teddyos/capabilities.json")
USER_CONF = Path(
    os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")
) / "teddyos" / "capabilities.json"

# Order is screen order. Only capabilities that are actually wired to something
# appear here — a switch for a feature with no backend is the theatre §2.2 is
# about, and it costs nothing to leave it out until the backend exists.
#
# Every row carries TWO sets of words. core/src/level.rs promises the Guided
# level is "plain language", and for a while that promise was broken: Guided
# and Advanced showed the same engineer prose and differed only in whether the
# wire name appeared. Someone who has never heard of a namespace was being told
# their search "runs in a namespace with no network interface at all", which is
# precise, true, and useless to them.
#
# The plain wording is not a softer claim. It says exactly the same thing —
# "teddyOS genuinely cannot reach the internet" is the same fact as
# PrivateNetwork=yes. Simplifying the language must never simplify the promise,
# because the whole argument of docs/thesis-os-doc-v01.md is that a permission
# screen which overstates itself is worse than none.
CAPABILITIES = [
    {
        "id": "search.query",
        "label": "Built-in help",
        "label_adv": "Built-in docs",
        "detail": "Search the guide that came with teddyOS",
        "detail_adv": "Search what ships with teddyOS",
        "enforced": False,
        "badge": "on/off only",
        "badge_adv": "not a restriction",
        # These three appear on one screen, under three switches, while
        # someone is trying to make three yes/no decisions. At 97 words between
        # them they stopped being reassurance and became something to scroll
        # past — and copy nobody reads protects nobody. Same promises, a third
        # of the words. The long-form reasoning lives in the docs, not in front
        # of a person mid-decision.
        "why": "A switch for a feature, not a lock — the help files are on "
               "this computer either way.",
        "why_adv": "The corpus ships in the image and is readable by anything "
                   "on this machine. This switch turns the feature off; it "
                   "does not contain it.",
        "default": True,
    },
    {
        "id": "workspace.index",
        "label": "Your files",
        "label_adv": "Your files",
        "detail": "Let teddyOS search folders you choose",
        "detail_adv": "Search folders you choose",
        "enforced": True,
        "badge": "the system enforces this",
        "badge_adv": "enforced",
        "why": "It can only look inside the folders you pick. The rest of "
               "your computer is out of reach — enforced, not promised.",
        "why_adv": "The indexer runs with BindReadOnlyPaths limited to the "
                   "chosen folders; a path outside them cannot be named.",
        "default": False,
    },
    {
        "id": "portal.sync",
        "label": "Search the web",
        "label_adv": "Online search",
        "detail": "Look things up on teddysearch.com",
        "detail_adv": "Ask teddysearch.com",
        "enforced": True,
        "badge": "the system enforces this",
        "badge_adv": "enforced",
        "why": "The only thing here that uses the internet. Off, it has no "
               "way out at all. On, what you type still never leaves this "
               "computer.",
        "why_adv": "PrivateNetwork=yes when denied — loopback only, so a "
                   "request fails to route rather than failing a policy check. "
                   "Granted, the full teddysearch corpus is reachable via "
                   "teddyos-search --sync.",
        "default": False,
    },
    {
        "id": "diagnostics.share",
        "label": "Help improve teddyOS",
        "label_adv": "Diagnostics sharing",
        "detail": "Allow system logs to be collected so we can fix problems",
        "detail_adv": "diagnostics.share — local log snapshots",
        "enforced": True,
        "badge": "the system enforces this",
        "badge_adv": "enforced",
        "why": "Off unless you turn it on. When on, teddyOS may save system "
               "logs on this computer so problems can be studied. What you type "
               "in Search is never sent, and nothing is uploaded unless you "
               "choose to share a report later.",
        "why_adv": "When denied, teddyos-log-collect and the daily timer "
                   "no-op. When granted, snapshots land under "
                   "/var/log/teddyos/snapshots. No ambient network upload — "
                   "sharing a snapshot is a separate, explicit step.",
        "default": False,
    },
]


def text(cap: dict, field: str, guided: bool) -> str:
    """Pick the plain or the precise wording for a row."""
    return cap[field] if guided else cap.get(f"{field}_adv", cap[field])


DEFAULTS = {c["id"]: c["default"] for c in CAPABILITIES}

# --- experience level -------------------------------------------------------
# Ported from core/src/level.rs, and the doc comment there is the contract:
# "Privacy-first default grants stay the same at every level; only copy density
# and Caps blurbs adapt." A level that quietly granted more would be a setting
# that trades your privacy for your convenience without saying so.
LEVELS = [
    {
        "id": "guided",
        "label": "Guided",
        "detail": "Simple words and more explanation — recommended.",
    },
    {
        "id": "advanced",
        "label": "Advanced",
        "detail": "Shorter wording. Technical names where helpful.",
    },
]
DEFAULT_LEVEL = "guided"

# --- skills -----------------------------------------------------------------
# The catalogue from core/src/skills.rs. These are the "different needs" part:
# two people with identical capability grants still want different playbooks,
# and a trader has no use for the ones a writer lives in.
#
# Every entry here is a markdown playbook, not code. A skill cannot grant
# itself a capability — it runs under whatever was granted on the previous
# screen or it does not run.
SKILLS = [
    {"id": "agent-plan-act", "label": "Getting things done",
     "label_adv": "Plan and act",
     "detail": "Take a job, work out the steps, do them",
     "detail_adv": "Break a goal into steps, then run them", "default": True},
    {"id": "knowledge-search", "label": "Finding things",
     "label_adv": "Knowledge search",
     "detail": "Look through whatever you allowed above",
     "detail_adv": "Search across everything you granted", "default": True},
    {"id": "capability-safe-tools", "label": "Staying inside the lines",
     "label_adv": "Safe tools",
     "detail": "Never uses anything you did not allow",
     "detail_adv": "Tool use that stays inside your grants", "default": True},
    {"id": "system-health-check", "label": "Checking the computer",
     "label_adv": "System health",
     "detail": "Notice when something is wrong with the machine",
     "detail_adv": "Check the machine is behaving", "default": False},
    {"id": "inbox-brief", "label": "Catching up on email",
     "label_adv": "Inbox brief",
     "detail": "Tell you what arrived, briefly",
     "detail_adv": "Summarise what arrived", "default": False},
    {"id": "email-triage", "label": "Sorting email",
     "label_adv": "Email triage",
     "detail": "Sort it, and write replies for you to check",
     "detail_adv": "Sort and draft replies", "default": False},
    {"id": "teddy-portals", "label": "Research and writing",
     "label_adv": "teddy portals",
     "detail": "Dig through teddysearch for a topic",
     "detail_adv": "Research and writing from teddysearch", "default": False},
    {"id": "market-portals", "label": "Markets and investing",
     "label_adv": "Market portals",
     "detail": "Prices, positioning and market data",
     "detail_adv": "Quotes, positioning, market data", "default": False},
]
DEFAULT_SKILLS = [s["id"] for s in SKILLS if s["default"]]


def _read(path: Path) -> dict:
    try:
        with path.open() as fh:
            data = json.load(fh)
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        # A corrupt or unreadable grants file must not read as "everything
        # granted". Falling back to defaults fails closed for every capability
        # whose default is off, which is every capability that leaves the
        # machine or touches the user's files.
        return {}


def load() -> dict:
    """Effective grants. User file wins; missing keys fall back to defaults."""
    merged = dict(DEFAULTS)
    for path in (SYSTEM_CONF, USER_CONF):
        granted = _read(path).get("granted", {})
        for key, value in granted.items():
            if key in merged and isinstance(value, bool):
                merged[key] = value
    return merged


def workspace_paths() -> list[str]:
    """Folders the user chose for workspace.index. Empty unless granted."""
    if not load().get("workspace.index"):
        return []
    paths = _read(USER_CONF).get("workspace_paths", [])
    return [p for p in paths if isinstance(p, str) and Path(p).is_dir()]


def level() -> str:
    """Experience level. Affects copy only, never grants."""
    value = _read(USER_CONF).get("level")
    return value if value in {l["id"] for l in LEVELS} else DEFAULT_LEVEL


def is_guided() -> bool:
    return level() == "guided"


def skills() -> list[str]:
    """Enabled skill playbooks."""
    data = _read(USER_CONF)
    if "skills" not in data:
        return list(DEFAULT_SKILLS)
    valid = {s["id"] for s in SKILLS}
    return [s for s in data["skills"] if s in valid]


def save(granted: dict, workspace: list[str] | None = None,
         level_id: str | None = None, enabled_skills: list[str] | None = None) -> Path:
    """Write the user's choices. Returns the path written."""
    USER_CONF.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "version": 2,
        "granted": {k: bool(granted.get(k, DEFAULTS[k])) for k in DEFAULTS},
        "workspace_paths": list(workspace or []),
        "level": level_id if level_id in {l["id"] for l in LEVELS} else DEFAULT_LEVEL,
        "skills": list(enabled_skills if enabled_skills is not None else DEFAULT_SKILLS),
    }
    # Write-then-rename: a half-written grants file parses as no grants, and a
    # user who loses power mid-save should not silently lose the network switch
    # they just turned on — or worse, keep one they just turned off.
    tmp = USER_CONF.with_suffix(".json.tmp")
    with tmp.open("w") as fh:
        json.dump(payload, fh, indent=2)
        fh.write("\n")
    tmp.replace(USER_CONF)
    return USER_CONF


def is_configured() -> bool:
    """True once the setup screen has been answered at least once."""
    return USER_CONF.exists()


def diagnostics_share_allowed() -> bool:
    """True when any configured user granted diagnostics.share.

    The log collector and its timer run as root (or as a service user), so
    they cannot rely on load() alone — that reads *this* process's home.
    A consent given by the desktop user must still be visible to them.
    """
    if load().get("diagnostics.share"):
        return True
    homes: list[Path] = []
    home_root = Path("/home")
    if home_root.is_dir():
        homes.extend(p for p in home_root.iterdir() if p.is_dir())
    # Also the service's own home, and root, for odd setups.
    for extra in (Path.home(), Path("/root")):
        if extra not in homes:
            homes.append(extra)
    for home in homes:
        conf = home / ".config" / "teddyos" / "capabilities.json"
        try:
            data = json.loads(conf.read_text())
        except (OSError, ValueError):
            continue
        granted = data.get("granted", {})
        if isinstance(granted, dict) and granted.get("diagnostics.share") is True:
            return True
    return False
