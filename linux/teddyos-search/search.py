"""Search across three sources, each behind its own grant.

Sources, in the order a result is worth having:

  workspace   the user's own files, indexed locally
  portal      teddysearch.com, via GET /api/search-index
  builtin     the corpus that ships in the image

The portal is the interesting one and the reason this file exists. teddysearch
does not expose a query endpoint — `/api/search` is a JSON 404, and `/api` is
the Super Intelligence market API, which is a different service sharing the
host. What it does expose is `/api/search-index`: the whole index, 433 entries
and about 150 KB, each carrying a prebuilt `search_text`.

That shape decides the design. Ranking happens here, on the machine, over a
cached copy — which means one network call per cache period rather than one per
keystroke, and means a typed query is never sent anywhere. The privacy property
is a side effect of the API's shape rather than something we had to negotiate
for, but it is worth stating plainly on the setup screen: teddyOS tells
teddysearch that it wants the index. It never tells it what you searched for.
"""

from __future__ import annotations

import json
import math
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from stat import S_ISREG
from pathlib import Path

import caps

PORTAL_URL = os.environ.get(
    "TEDDYOS_PORTAL_URL", "https://teddysearch.com/api/search-index"
)
PORTAL_HOST = "teddysearch.com"
CACHE_DIR = Path(
    os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")
) / "teddyos"
PORTAL_CACHE = CACHE_DIR / "portal-index.json"
# Six hours. The index changes when Theo publishes, which is daily at most, so
# anything shorter spends the user's bandwidth to learn nothing.
PORTAL_TTL = int(os.environ.get("TEDDYOS_PORTAL_TTL", 6 * 3600))
PORTAL_TIMEOUT = 10

# --- the portal proper ------------------------------------------------------
# The index above is a directory of teddysearch's own pages. The portal is the
# search engine behind it: a real crawl, `{t, u, c, b}` per document with the
# body text included, currently 67.7 MB.
#
# Granting portal.sync is what connects the two. But 67.7 MB is not something
# to pull silently behind someone's first search — on a phone tether that is a
# noticeable amount of their month, and a search that takes four minutes the
# first time reads as broken. So the corpus is fetched by an explicit
# `teddyos-search --sync`, and until it exists the portal answers from the
# small index alone and says so.
#
# delta.json is what makes refreshing cheap: 156 KB carrying the crawl
# timestamp plus added/removed/changed lists. Comparing its `at` against the
# cached corpus's `crawled_at` decides whether the big download is needed at
# all, so a daily check costs 156 KB rather than 67 MB.
PORTAL_CORPUS_URL = os.environ.get(
    "TEDDYOS_PORTAL_CORPUS", "https://teddysearch.com/tsearch/corpus.json"
)
PORTAL_DELTA_URL = os.environ.get(
    "TEDDYOS_PORTAL_DELTA", "https://teddysearch.com/tsearch/delta.json"
)
PORTAL_CORPUS_CACHE = CACHE_DIR / "portal-corpus.json"
# Paths that are not a document. 13 corpus entries — the 13F holdings pages —
# record the SEARCH PAGE as their address, so every one of them "opens" and
# lands you back where you started. Normalising them produces a perfectly
# valid URL, which is why they read as working links right up until you click.
#
# This is a crawler bug and the real fix is upstream, in whatever writes
# corpus.json. Until then a result that cannot go anywhere is routed to a
# search for its own title, which at least lands near the content instead of
# on the page you just left.
NON_DOCUMENT_PATHS = {"", "/", "/tsearch", "/tsearch/"}
PORTAL_CORPUS_TIMEOUT = 600

BUILTIN_CORPUS = Path(
    os.environ.get("TEDDYOS_CORPUS", "/usr/share/teddyos/corpus.json")
)

_WORD = re.compile(r"[a-z0-9]+")
# owner/repo and folder paths the user types as one unit ("iakovos/trading").
# Emitted alongside the split words so a path-shaped query can still hit a
# path-shaped filename without requiring both halves in the body.
_PATHISH = re.compile(r"[a-z0-9]+(?:/[a-z0-9._-]+)+")

# BM25. k1 controls how fast repeated terms stop helping; b how hard length is
# normalised. These are the standard values and there is no reason to invent
# our own — the previous scorer did, and produced a flat band of ~0.05 scores
# where ordering was effectively arbitrary.
BM25_K1 = 1.5
BM25_B = 0.75
TITLE_REPEAT = 5

# People type goals into a field that says "Ask anything". The ranking index
# only understands keywords. Leading intent is stripped so
# "i wanna work on iakovos/trading" ranks as "iakovos trading", not as a
# four-term query where "wanna" and "work" drown the rare name.
#
# Applied once, at the front of the query only — "work on trading" mid-sentence
# is content, not a wrapper.
_INTENT_PREFIX = re.compile(
    r"""^
    (?:
        (?:hey\s+|hi\s+|please\s+)?
        (?:i\s+)?
        (?:wanna|want\s+to|need\s+to|gonna|gotta|would\s+like\s+to)\s+
    )?
    (?:
        work\s+on|working\s+on|
        open|find|search\s+for|look\s+(?:up|for)|
        show(?:\s+me)?|go\s+to|take\s+me\s+to|
        start|continue|resume|
        help\s+(?:me\s+)?(?:with|on|to)?|
        # Everyday action goals (not project folders)
        respond\s+to|reply\s+to|answer|
        check(?:\s+my|\s+on)?|catch\s+up\s+on|go\s+through|
        handle|deal\s+with|clear|
        draft|write\s+(?:a\s+)?(?:reply|response|message)
    )
    \s+
    """,
    re.IGNORECASE | re.VERBOSE,
)

# Task-shaped asks that should open Get help even when the intent regex
# barely matches, or when the "subject" is clearly not a folder name.
_TASK_GOAL_RE = re.compile(
    r"""
    \b(
        respond\s+to|reply\s+to|answer\s+my|
        linkedin\s+messages?|linkedin\s+inbox|linkedin\s+dms?|
        whatsapp\s+messages?|whatsapp\s+chats?|whats\s*app|
        (?:my\s+)?(?:messages?|emails?|inbox)|
        inmail|connection\s+requests?|
        draft\s+(?:a\s+)?reply|write\s+(?:a\s+)?reply
    )\b
    """,
    re.IGNORECASE | re.VERBOSE,
)


def normalise_url(raw: str) -> str:
    """Turn a corpus URL into something a browser can actually open.

    The corpus carries three shapes and only one of them survives naive
    prefixing:

        https://example.com/x   absolute            — use as-is
        en.wikipedia.org/wiki/X  host-relative      — needs a scheme
        /tsearch/docs/notes/x    SITE-relative      — needs the HOST too

    `f"https://{raw}"` handles the middle case and silently destroys the last
    one: it produces `https:///tsearch/...` with an empty authority, so the
    browser reads the first path segment as a hostname and reports "We can't
    connect to the server at tsearch". 1141 of 12453 documents — about one
    result in eleven — are site-relative, so this was a scary error page on a
    regular basis, from a machine whose whole pitch is that it answers you.
    """
    raw = (raw or "").strip()
    if not raw:
        return ""
    if raw.startswith(("http://", "https://")):
        return raw
    if raw.startswith("//"):
        return f"https:{raw}"
    if raw.startswith("/"):
        return f"https://{PORTAL_HOST}{raw}"
    return f"https://{raw}"


# Words that carry no signal about WHICH document you want. The field says
# "Ask anything", so people ask things — "can you tell me about my meetings" is
# seven terms of which one is the question. Scored literally, the six filler
# words dominate: almost every document contains "can", "you", "about", so the
# coverage rule either demands all seven (matching nothing) or falls back to a
# score floor where ordering is noise.
#
# Stripped for RANKING only. The full text still goes to the web escape hatch,
# because a search engine on the other end may well handle the question form
# better than a keyword index can.
STOPWORDS = frozenset("""
a an the and or but if then than that this these those of in on at to from by
for with about into over after is are was were be been being do does did doing
have has had having i me my we us our you your he him his she her it its they
them their what which who whom whose when where why how can could should would
will shall may might must please tell show find get give know like want need
wanna gonna gotta
""".split())

_HYPHENATED = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)+")


def focus_query(text: str) -> str:
    """Strip leading chat-intent so ranking sees the subject, not the wrapper.

    "i wanna work on iakovos/trading" → "iakovos/trading"
    "open ~/Desktop/notes.md" → "~/Desktop/notes.md"
    "immigration paradise" is unchanged (no leading intent verb).
    """
    stripped = text.strip()
    if not stripped:
        return stripped
    focused = _INTENT_PREFIX.sub("", stripped, count=1).strip()
    # Never return empty: if the whole query was intent ("show me"), keep the
    # original so query_terms can still fall back to stopwords.
    return focused or stripped


def is_work_goal(text: str) -> bool:
    """True when the query is a goal ("work on X"), not a keyword search.

    The Search window uses this to offer AI tools instead of (only) document
    hits — "i wanna work on iakovos-trading" should open a picker, not a
    Wikipedia list. Same for everyday tasks like "respond to my LinkedIn
    messages" — Get help, not a random corpus hit.
    """
    stripped = (text or "").strip()
    if not stripped:
        return False
    if _INTENT_PREFIX.match(stripped):
        return True
    return bool(_TASK_GOAL_RE.search(stripped))


def is_freeform_help_goal(text: str) -> bool:
    """True when the goal is a task (messages, email), not a project folder.

    Freeform goals still use Get help, but against the home folder with the
    full natural-language ask as the prompt — never a GitHub clone dead-end.
    """
    stripped = (text or "").strip()
    if not stripped or not is_work_goal(stripped):
        return False
    if _TASK_GOAL_RE.search(stripped):
        return True
    subject = focus_query(stripped)
    low = (subject or "").lower()
    if not low:
        return False
    # Multi-word subjects that look like tasks, not slugs like "my-app"
    words = low.split()
    if len(words) >= 2 and any(
        w in low
        for w in (
            "message", "messages", "email", "emails", "inbox", "linkedin",
            "reply", "replies", "respond", "dm", "dms", "inmail", "slack",
            "whatsapp", "calendar", "meeting", "meetings", "mail",
        )
    ):
        return True
    return False


_LINKEDIN_MSG_RE = re.compile(
    r"""
    \b(
        linkedin\s+(messages?|inbox|dms?|inmail|messaging)|
        (reply|respond|answer|check|clear|draft)\s+(to\s+)?(my\s+)?linkedin|
        (reply|respond|draft)\s+(to\s+)?(my\s+)?linkedin\s+messages?
    )\b
    """,
    re.IGNORECASE | re.VERBOSE,
)


def _fold_channel_typos(low: str) -> str:
    """Fold common channel misspellings so intent still fires.

    People type “linkdin” and “watsapp” constantly; treating those as
    unknown lookups is worse than a soft fold. Only whole-token-ish
    replacements — never rewrite real English mid-word.
    """
    low = re.sub(r"\blinkd?in\b", "linkedin", low)
    low = re.sub(r"\blinkedin\b", "linkedin", low)  # no-op normalise
    low = re.sub(r"\b(watsapp|whatsap|whatapp|whatsaap)\b", "whatsapp", low)
    low = low.replace("whats app", "whatsapp")
    return low


def is_linkedin_messages_goal(text: str) -> bool:
    """True when the person wants AI help with LinkedIn messaging / replies.

    Requires an explicit LinkedIn signal — bare “reply to my messages” is not
    LinkedIn (that may be WhatsApp, email, or generic inbox help).
    """
    stripped = (text or "").strip()
    if not stripped:
        return False
    low = _fold_channel_typos(stripped.lower())
    if "linkedin" not in low:
        return False
    if any(
        w in low
        for w in (
            "message", "messages", "inbox", "dm", "dms", "inmail",
            "reply", "replies", "respond", "answer", "draft", "messaging",
        )
    ):
        return True
    return bool(_LINKEDIN_MSG_RE.search(low))


# Action prompt when we open Get help for LinkedIn replies.
# LinkedIn has no public messaging API for the guest — we open the inbox and
# AI drafts paste-ready replies (never auto-send).
LINKEDIN_REPLY_PROMPT = (
    "I want AI help replying to my LinkedIn messages right now. LinkedIn "
    "Messaging should be open (or about to open) on this computer.\n\n"
    "You are my reply co-pilot. Clear the inbox one thread at a time:\n"
    "1) In one short sentence, ask me to paste the latest message (or short "
    "thread) I need to answer.\n"
    "2) When I paste, draft a warm, professional reply I can paste back into "
    "LinkedIn — under ~90 words, no hashtags, no fake claims about my work.\n"
    "3) Offer two options if useful: (A) short & friendly (B) a bit more formal.\n"
    "4) End each draft with “Ready to paste — send the next message when you want.”\n"
    "5) Never claim you sent anything. You only draft; I paste and send.\n\n"
    "Start now with step 1."
)


def linkedin_reply_action_prompt(user_query: str = "") -> str:
    """Full model prompt for a LinkedIn reply session."""
    q = (user_query or "").strip()
    if q and q.lower() not in LINKEDIN_REPLY_PROMPT.lower():
        return f"{LINKEDIN_REPLY_PROMPT}\n\nTheir words: {q}"
    return LINKEDIN_REPLY_PROMPT


_WHATSAPP_MSG_RE = re.compile(
    r"""
    \b(
        whats\s*app\s+(messages?|chats?|inbox|dms?|messaging|replies)?|
        (reply|respond|answer|check|clear|draft)\s+(to\s+)?(my\s+)?whats\s*app|
        (reply|respond|draft)\s+(to\s+)?(my\s+)?whats\s*app(\s+messages?)?|
        draft\s+whats\s*app
    )\b
    """,
    re.IGNORECASE | re.VERBOSE,
)


def is_whatsapp_messages_goal(text: str) -> bool:
    """True when the person wants AI help with WhatsApp messaging / replies.

    Requires an explicit WhatsApp signal.
    """
    stripped = (text or "").strip()
    if not stripped:
        return False
    low = _fold_channel_typos(stripped.lower())
    if "whatsapp" not in low:
        return False
    if any(
        w in low
        for w in (
            "message", "messages", "chat", "chats", "inbox",
            "dm", "dms", "reply", "replies", "respond", "answer",
            "check", "draft", "help", "ai",
        )
    ):
        return True
    # Bare “whatsapp” / “my whatsapp” still means open chat + AI help
    if low in ("whatsapp", "my whatsapp", "open whatsapp"):
        return True
    if any(w in low for w in ("wanna", "want", "open", "read", "catch", "use")):
        return True
    return bool(_WHATSAPP_MSG_RE.search(low))


# Same product model as LinkedIn: open Web WhatsApp, AI drafts paste-ready
# replies (never auto-send). No WhatsApp Business API in the guest.
WHATSAPP_REPLY_PROMPT = (
    "I want AI help replying to my WhatsApp messages right now. WhatsApp Web "
    "should be open (or about to open) on this computer.\n\n"
    "You are my reply co-pilot. Clear chats one thread at a time:\n"
    "1) In one short sentence, ask me to paste the latest message (or short "
    "thread) I need to answer.\n"
    "2) When I paste, draft a natural reply I can paste back into WhatsApp — "
    "under ~80 words, match their tone (casual if they are casual), no "
    "corporate fluff, no fake claims.\n"
    "3) Offer two options if useful: (A) short & chill (B) a bit clearer / "
    "more careful.\n"
    "4) End each draft with “Ready to paste — send the next message when you want.”\n"
    "5) Never claim you sent anything. You only draft; I paste and send.\n\n"
    "Start now with step 1."
)


def whatsapp_reply_action_prompt(user_query: str = "") -> str:
    """Full model prompt for a WhatsApp reply session."""
    q = (user_query or "").strip()
    if q and q.lower() not in WHATSAPP_REPLY_PROMPT.lower():
        return f"{WHATSAPP_REPLY_PROMPT}\n\nTheir words: {q}"
    return WHATSAPP_REPLY_PROMPT


_EMAIL_MSG_RE = re.compile(
    r"""
    \b(
        (reply|respond|answer|check|clear|draft|catch\s+up)\s+
            (to\s+)?(my\s+)?(emails?|gmail|mail|inbox)|
        (emails?|gmail|mail)\s+(replies?|messages?|help|drafts?)|
        my\s+(emails?|gmail|inbox)|
        help\s+with\s+(my\s+)?(emails?|gmail|inbox)
    )\b
    """,
    re.IGNORECASE | re.VERBOSE,
)


def is_email_messages_goal(text: str) -> bool:
    """True when the person wants AI help with their email *inbox*.

    Compose-from-scratch asks (“write a meeting follow-up email after a job
    interview”) are freeform writing, not the open-inbox + draft-replies path.
    Inbox language (reply / check / clear / catch up / gmail) still qualifies.
    """
    stripped = (text or "").strip()
    if not stripped:
        return False
    low = stripped.lower()
    # Explicit channel words for email.
    if not any(w in low for w in ("email", "emails", "gmail", "mail", "inbox")):
        return False

    # Compose a new message from a brief — not "clear my inbox".
    compose_shaped = any(
        p in low
        for p in (
            "write a", "write an", "write me", "draft a", "draft an",
            "compose a", "compose an", "follow-up email", "follow up email",
            "thank you email", "thank-you email", "cold email",
            "cover letter", "outreach email",
        )
    )
    inbox_shaped = any(
        w in low
        for w in (
            "reply", "replies", "respond", "answer", "check", "clear",
            "catch", "inbox", "unread", "triage",
        )
    ) or bool(re.search(r"\b(my\s+)?(emails?|gmail|mail)\b", low) and any(
        w in low for w in ("help", "read", "draft", "messages")
    ))
    if compose_shaped and not inbox_shaped:
        return False

    # Prefer messaging language over pure search.
    if any(
        w in low
        for w in (
            "reply", "replies", "respond", "answer", "draft", "check",
            "clear", "catch", "help", "read", "write",
        )
    ):
        # “write” alone is too broad when compose_shaped already bailed;
        # remaining “write” hits are inbox-ish (“write replies”).
        return True
    return bool(_EMAIL_MSG_RE.search(stripped))


# Same product model as LinkedIn/WhatsApp: open Email app, AI drafts only.
# Headless Claude must NOT use Gmail MCP tools (permission prompts break -p mode).
EMAIL_REPLY_PROMPT = (
    "I want AI help replying to my email right now. Email (webmail) should be "
    "open or about to open on this computer.\n\n"
    "CRITICAL rules for this session:\n"
    "- Do NOT use Gmail tools, MCP, search_threads, or any inbox API.\n"
    "- Do NOT mention terminal, claude, settings.json, permissions files, "
    "or developer setup.\n"
    "- You only draft; I paste and send. Never claim you sent mail.\n\n"
    "You are my email reply co-pilot:\n"
    "1) In one short sentence, ask me to paste the email (or short thread) "
    "I need to answer.\n"
    "2) When I paste, draft a clear reply I can paste into my mail app — "
    "under ~120 words, match their tone, no fake claims.\n"
    "3) Offer two options if useful: (A) short & friendly (B) a bit more formal.\n"
    "4) End each draft with “Ready to paste — send the next one when you want.”\n\n"
    "Start now with step 1 only. Plain language. No Markdown."
)


def email_reply_action_prompt(user_query: str = "") -> str:
    """Full model prompt for an email reply session."""
    q = (user_query or "").strip()
    if q and q.lower() not in EMAIL_REPLY_PROMPT.lower():
        return f"{EMAIL_REPLY_PROMPT}\n\nTheir words: {q}"
    return EMAIL_REPLY_PROMPT


def resolve_project_dirs(name: str, roots: list[Path] | None = None) -> list[Path]:
    """Find local project folders matching a goal subject.

    Looks under home and the granted workspace roots. Prefers an exact
    directory name match (case-insensitive), then a unique prefix match,
    then any depth-2 folder whose name contains the slug.
    """
    slug = (name or "").strip().strip("/").replace("\\", "/")
    if not slug:
        return []
    # "iakovos/trading" → look for iakovos-trading and iakovos/trading
    variants = {slug, slug.replace("/", "-"), slug.replace("-", "/")}
    variants |= {v.lower() for v in list(variants)}

    search_roots: list[Path] = []
    home = Path.home()
    search_roots.append(home)
    for r in roots if roots is not None else caps.workspace_paths():
        p = Path(r)
        if p not in search_roots:
            search_roots.append(p)

    exact: list[Path] = []
    fuzzy: list[Path] = []
    seen: set[Path] = set()

    def _add(bucket: list[Path], path: Path) -> None:
        try:
            resolved = path.resolve()
        except OSError:
            resolved = path
        if not resolved.is_dir() or resolved in seen:
            return
        # Never offer home itself as "the project".
        if resolved == home.resolve():
            return
        seen.add(resolved)
        bucket.append(resolved)

    for root in search_roots:
        if not root.is_dir():
            continue
        # Direct children first — the common case (~/iakovos-trading).
        try:
            children = list(root.iterdir())
        except OSError:
            continue
        for child in children:
            if not child.is_dir() or child.name.startswith("."):
                continue
            n = child.name
            nl = n.lower()
            if nl in variants or n in variants:
                _add(exact, child)
            elif any(v in nl for v in variants if len(v) >= 3):
                _add(fuzzy, child)
        # One level deeper: ~/Desktop/iakovos-trading, ~/projects/foo
        for child in children:
            if not child.is_dir() or child.name.startswith("."):
                continue
            try:
                grand = list(child.iterdir())
            except OSError:
                continue
            for g in grand:
                if not g.is_dir() or g.name.startswith("."):
                    continue
                nl = g.name.lower()
                if nl in variants or g.name in variants:
                    _add(exact, g)
                elif any(v in nl for v in variants if len(v) >= 3):
                    _add(fuzzy, g)

    return exact or fuzzy


def query_terms(text: str) -> list[str]:
    """Tokens worth ranking on. Falls back to the raw tokens when a query is
    nothing but stopwords, because returning nothing for "how do i" is worse
    than returning something loosely related — and a user who typed only
    filler is going to rephrase either way.

    Path-shaped tokens (`iakovos/trading`) are intentionally left out here.
    They almost never appear in the web corpus, so counting them as required
    coverage terms made every portal result look like a partial match and let
    common words win. Workspace search still uses them via path_terms().
    """
    tokens = tokenize(focus_query(text))
    meaningful = [
        t for t in tokens
        if t not in STOPWORDS and len(t) > 1 and "/" not in t
    ]
    return meaningful or [t for t in tokens if t not in STOPWORDS] or tokens


def path_terms(text: str) -> list[str]:
    """Slash-joined segments from the focused query, for workspace path boosts."""
    return [m.group(0) for m in _PATHISH.finditer(focus_query(text).lower())]


def tokenize(text: str) -> list[str]:
    """Words, plus the joined form of anything hyphenated or path-shaped.

    "H-1B" splits into h / 1 / b, so a search for "h1b" matched nothing and the
    document titled "H-1B visa" ranked below "Visa Inc". Emitting BOTH forms
    means h-1b is findable as "h-1b" and as "h1b" without deciding which one
    the user will type. Same for covid-19/covid19, e-mail/email, 401-k/401k.

    "iakovos/trading" also yields the path token so a local file under that
    folder can match the path shape; query_terms() drops the slash form so the
    web index is not forced to cover a token it never contains.
    """
    lowered = text.lower()
    tokens = _WORD.findall(lowered)
    tokens.extend(m.group(0).replace("-", "") for m in _HYPHENATED.finditer(lowered))
    for m in _PATHISH.finditer(lowered):
        path = m.group(0)
        tokens.append(path)
        # Also the last segment alone — "…/trading/README.md" should still
        # answer a query that ends in /trading.
        tail = path.rsplit("/", 1)[-1]
        if tail and tail not in tokens:
            tokens.append(tail)
    return tokens


@dataclass
class Result:
    title: str
    url: str
    snippet: str
    source: str
    score: float = 0.0
    category: str = ""
    # How many of the query's terms this document actually contained. Used to
    # cut the tail: see prune().
    matched: int = 0
    terms: int = 0


@dataclass
class Outcome:
    """What a search run produced, including what it could not do.

    `denied` is not an error list. A capability that is off is a decision the
    user made, and the UI reports it as a state rather than a failure — but it
    has to be reported, because "no results" and "no results because online
    search is off" look identical and only one of them is worth acting on.
    """
    results: list[Result] = field(default_factory=list)
    denied: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    # Not errors and not denials: things that are working as configured but
    # where the user could get more by doing something.
    notes: list[str] = field(default_factory=list)
    portal_age: float | None = None
    # Recipes / “near me” / order-food — open web is the honest first answer.
    web_first: bool = False


# Everyday asks the Guide crawl answers poorly (food recipes, local delivery).
# Detected here so ranking can demote weak portal noise and the UI can put the
# web row first. Conservative on bare "order" (not "order of magnitude").
_WEB_FIRST_RE = re.compile(
    r"""
    \b(
        recipes?|
        near\s+me|nearby|
        how\s+to\s+(make|cook|bake|prepare|grill|fry|roast)|
        how\s+do\s+i\s+(make|cook|bake|prepare)|
        (order|get)\s+(me\s+)?(a\s+|some\s+)?(
            pizza|food|sushi|tacos?|burger|coffee|ramen|chinese|thai|
            indian|mexican|delivery|takeout|take-?out
        )|
        (food\s+)?delivery|takeout|take-?out|uber\s*eats|doordash|grubhub|
        restaurants?\s+near|open\s+now
    )\b
    """,
    re.IGNORECASE | re.VERBOSE,
)


def is_web_first_goal(text: str) -> bool:
    """True when the open web should lead, not Guide BM25 noise.

    Recipe / near-me / delivery phrasing. Bare food names stay normal lookup
    so offline food Help seed and real wiki pages can still win.
    """
    stripped = (text or "").strip()
    if not stripped:
        return False
    return bool(_WEB_FIRST_RE.search(stripped))


# Content words that do not prove a Guide hit is about the dish (they match
# almost every cooking-adjacent wiki page and recreated the Focaccia problem).
_WEB_FIRST_FILLER = frozenset({
    "recipe", "recipes", "how", "to", "make", "cook", "bake", "prepare",
    "grill", "fry", "roast", "near", "me", "nearby", "order", "get", "best",
    "easy", "food", "delivery", "takeout", "take", "out", "restaurant",
    "restaurants", "open", "now", "doordash", "uber", "eats", "grubhub",
    "some", "a", "an", "the", "for", "with", "and", "or",
})


def _title_has_content_token(title: str, content: list[str]) -> bool:
    """True if a dish/place token appears as a whole word in the title.

    Substring match let “smelt” keep “Smelting” and similar false friends.
    """
    title_l = (title or "").lower()
    for t in content:
        if re.search(rf"\b{re.escape(t)}\b", title_l):
            return True
    return False


def _filter_web_first_results(
    results: list[Result], query: str = "",
) -> list[Result]:
    """Keep food Help + honest Guide; drop OS docs and false-friend wiki.

    “pizza recipe” used to rank Focaccia; “smelt recipe” ranked Smelting;
    “how to make Ashure” kept Desktop UTM because “make” hits OS seed text.
    """
    content = [
        t for t in query_terms(query)
        if t not in _WEB_FIRST_FILLER and len(t) > 2
    ]
    keep: list[Result] = []
    for r in results:
        src = (r.source or "").lower()
        title = r.title or ""
        cat = (r.category or "").lower()

        if src == "built-in":
            # Web-first tip always; food cards only when the dish is in the
            # title (else “how to make Ashure” listed every food card).
            title_l = title.lower()
            if "recipe" in title_l or "near me" in title_l:
                keep.append(r)
            elif content and _title_has_content_token(title, content):
                keep.append(r)
            continue

        if src in ("your files", "workspace"):
            # Only keep local files that look about the dish (not a probe JSON
            # that happens to list the query string).
            if content and _title_has_content_token(title, content):
                keep.append(r)
            continue

        if src not in ("portal", "teddysearch", "web"):
            continue
        if content and _title_has_content_token(title, content):
            keep.append(r)
    return keep


def _score(query_tokens: list[str], text: str, title: str = "") -> float:
    """Rank a document against a query.

    Three things decide the order, and the first matters most:

    COVERAGE. How many of the query's terms the document contains at all.
    Without it, term contributions just sum and the length divisor lets a short
    document matching ONE term beat a long one matching every term — which is
    how a query for "immigration paradise" returned "Solitary confinement"
    above "Asylum seeker". Coverage is applied as a square, so a document
    matching half the query scores a quarter, not half.

    WHERE the match is. A term in the title is a document about that thing; the
    same term once in a 2000-word body is a document that mentions it. Title
    hits are worth 4x, and a title hit alone can outrank a body full of them.

    HOW WELL it matches. An exact token beats a prefix ("wheel" should find
    "wheels" without "w" finding everything), and repeated occurrences help
    with diminishing returns.

    Note on what this deliberately does NOT do: there is no IDF. Computing
    document frequency means a pass over 12k documents before scoring can
    start, and the earlier version of this docstring claimed rare terms were
    weighted more while the code did nothing of the sort. An honest simple
    scorer beats a comment describing one that was never written.
    """
    if not query_tokens:
        return 0.0

    body = tokenize(text)
    head = tokenize(title)
    if not body and not head:
        return 0.0

    body_counts: dict[str, int] = {}
    for token in body:
        body_counts[token] = body_counts.get(token, 0) + 1
    head_counts: dict[str, int] = {}
    for token in head:
        head_counts[token] = head_counts.get(token, 0) + 1

    # Gentle: sqrt over-punished long documents, and the corpus bodies vary
    # from a sentence to several pages.
    norm = math.sqrt(len(body)) if body else 1.0

    score = 0.0
    matched = 0
    for term in query_tokens:
        term_score = 0.0

        if term in head_counts:
            term_score += 4.0
        elif len(term) >= 3 and any(k.startswith(term) for k in head_counts):
            term_score += 1.5

        hit = body_counts.get(term, 0)
        if hit:
            term_score += 3.0 * (1.0 + math.log(hit)) / norm
        elif len(term) >= 3:
            partial = sum(v for k, v in body_counts.items() if k.startswith(term))
            if partial:
                term_score += 0.6 * (1.0 + math.log(partial)) / norm

        if term_score > 0:
            matched += 1
            score += term_score

    if not matched:
        return 0.0

    coverage = matched / len(query_tokens)
    return score * (coverage ** 2)


def _score_matched(
    query_tokens: list[str], text: str, title: str = ""
) -> tuple[float, int]:
    """Like _score, but also returns how many query terms hit.

    Needed so Result.matched/terms are set for prune() across every source,
    not only the BM25 portal corpus path.
    """
    if not query_tokens:
        return 0.0, 0
    # Recompute match count the same way _score does, without drifting.
    body = set(tokenize(text))
    head = set(tokenize(title))
    bag = body | head
    matched = 0
    for term in query_tokens:
        if term in bag or (
            len(term) >= 3 and any(k.startswith(term) for k in bag)
        ):
            matched += 1
    return _score(query_tokens, text, title), matched


# --- portal -----------------------------------------------------------------

def _cache_age() -> float | None:
    try:
        return time.time() - PORTAL_CACHE.stat().st_mtime
    except OSError:
        return None


def fetch_portal_index(force: bool = False) -> tuple[dict | None, str | None]:
    """Return (index, error). Serves cache when fresh, or when the network fails.

    A stale cache beats an empty result: the machine may be offline, and an
    index from this morning still answers most questions. The age is surfaced
    so the caller can say so rather than presenting old data as current.
    """
    age = _cache_age()
    if not force and age is not None and age < PORTAL_TTL:
        try:
            return json.loads(PORTAL_CACHE.read_text()), None
        except (OSError, ValueError):
            pass  # fall through and refetch

    req = urllib.request.Request(
        PORTAL_URL,
        headers={"User-Agent": "teddyOS/1.0 (+https://teddysearch.com)"},
    )
    try:
        with urllib.request.urlopen(req, timeout=PORTAL_TIMEOUT) as resp:
            raw = resp.read()
        index = json.loads(raw)
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        tmp = PORTAL_CACHE.with_suffix(".json.tmp")
        tmp.write_bytes(raw)
        tmp.replace(PORTAL_CACHE)
        return index, None
    except (urllib.error.URLError, OSError, ValueError) as exc:
        # PrivateNetwork=yes surfaces here as a DNS failure. That is the grant
        # working, not a fault, so the caller checks the grant before it treats
        # this as an error worth showing.
        if PORTAL_CACHE.exists():
            try:
                return (json.loads(PORTAL_CACHE.read_text()),
                        "Showing what was saved last time — this computer isn't online.")
            except (OSError, ValueError):
                pass
        return None, humanise(exc)


def search_portal(query: str, limit: int) -> tuple[list[Result], str | None, float | None]:
    index, error = fetch_portal_index()
    if index is None:
        return [], error, None

    tokens = query_terms(query)
    if not tokens:
        return [], None, _cache_age()

    out: list[Result] = []
    for entry in index.get("index", []):
        text = entry.get("search_text") or f"{entry.get('name','')} {entry.get('desc','')}"
        score, matched = _score_matched(tokens, text, entry.get("name", ""))
        if score <= 0:
            continue
        out.append(Result(
            title=entry.get("name", "(untitled)"),
            url=normalise_url(entry.get("path", "")),
            snippet=entry.get("desc", ""),
            source="teddysearch",
            score=score,
            category=entry.get("category", ""),
            matched=matched,
            terms=len(tokens),
        ))
    out.sort(key=lambda r: r.score, reverse=True)
    return prune(out)[:limit], error, _cache_age()


def humanise(exc: object) -> str:
    """Turn an exception into a sentence somebody can act on.

    These strings are rendered in the Search window, not just printed to a
    terminal, and the raw ones are Python's:

        <urlopen error [Errno -3] Temporary failure in name resolution>

    Which is accurate, useless, and alarming — it looks like the machine is
    broken when the actual news is "you are not online". Anyone who has never
    seen a traceback reads an errno as damage.

    Both the window and the CLI get the sentence. Keeping the raw string for
    terminal users was tempting and wrong: the two would drift, and the version
    that gets read least is the one that stays accurate least.
    """
    text = str(exc).lower()
    if "name resolution" in text or "nodename nor servname" in text \
            or "temporary failure" in text or "name or service not known" in text:
        return "This computer isn’t online, so the web wasn’t searched."
    if "timed out" in text or "timeout" in text:
        return "The web search took too long. Try again in a moment."
    if "connection refused" in text or "network is unreachable" in text \
            or "no route to host" in text:
        return "Couldn’t reach the internet. Check your connection and try again."
    if "certificate" in text or "ssl" in text:
        return "Couldn’t make a secure connection to the web search, so nothing was fetched."
    if "http error 4" in text or "http error 5" in text:
        return "The web search had a problem. Try again in a moment."
    return "Something went wrong reaching the web, so those results are missing."


def _get_json(url: str, timeout: int) -> tuple[object | None, str | None]:
    req = urllib.request.Request(
        url, headers={"User-Agent": "teddyOS/1.0 (+https://teddysearch.com)"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read()), None
    except (urllib.error.URLError, OSError, ValueError) as exc:
        return None, humanise(exc)


def portal_corpus_status() -> dict:
    """What we hold locally, and whether the portal has moved on.

    Reads only the cached corpus header, not its 67 MB of documents — a status
    check that had to parse the whole file would be too slow to run before
    every search, which is exactly when you want it.
    """
    status = {"present": False, "crawled_at": None, "docs": 0, "bytes": 0}
    try:
        status["bytes"] = PORTAL_CORPUS_CACHE.stat().st_size
        with PORTAL_CORPUS_CACHE.open("rb") as fh:
            head = fh.read(4096).decode("utf-8", "ignore")
        match = re.search(r'"crawled_at"\s*:\s*"([^"]+)"', head)
        status["present"] = True
        status["crawled_at"] = match.group(1) if match else None
    except OSError:
        pass
    return status


def sync_portal_corpus(force: bool = False, progress=None) -> tuple[bool, str]:
    """Download the portal corpus if it is missing or the crawl has moved on."""
    if not caps.load().get("portal.sync"):
        return False, "Online search is off — nothing was fetched."

    local = portal_corpus_status()
    if local["present"] and not force:
        delta, err = _get_json(PORTAL_DELTA_URL, PORTAL_TIMEOUT)
        if delta is None:
            return False, f"Couldn't check for updates. {err}"
        remote_at = (delta.get("latest") or {}).get("at")
        if remote_at and remote_at == local["crawled_at"]:
            return True, f"already current ({remote_at}, {local['bytes'] // 1_000_000} MB)"
        if progress:
            progress(f"portal crawled {remote_at}; local copy is {local['crawled_at']}")

    if progress:
        progress("downloading the portal corpus (this is the big one)")

    req = urllib.request.Request(
        PORTAL_CORPUS_URL,
        headers={"User-Agent": "teddyOS/1.0 (+https://teddysearch.com)"})
    tmp = PORTAL_CORPUS_CACHE.with_suffix(".json.part")
    try:
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(req, timeout=PORTAL_CORPUS_TIMEOUT) as resp, \
                tmp.open("wb") as out:
            total = 0
            while chunk := resp.read(1 << 20):
                out.write(chunk)
                total += len(chunk)
                if progress and total % (10 << 20) < (1 << 20):
                    progress(f"  {total // 1_000_000} MB")
        # Parse before publishing. A truncated download is still valid-looking
        # JSON prefix on disk, and a corpus that half-loads would silently
        # return fewer results forever.
        with tmp.open() as fh:
            data = json.load(fh)
        docs = len(data.get("docs", []))
        if docs == 0:
            tmp.unlink(missing_ok=True)
            return False, "downloaded corpus contained no documents — not installed"
        tmp.replace(PORTAL_CORPUS_CACHE)
        return True, f"portal corpus: {docs} documents, {total // 1_000_000} MB"
    except (urllib.error.URLError, OSError, ValueError) as exc:
        tmp.unlink(missing_ok=True)
        return False, f"The download didn't finish. {humanise(exc)}"


# Parsed once, kept. The corpus is 67 MB of JSON and takes seconds to parse;
# re-reading it per query is tolerable for a one-shot CLI and completely
# unusable behind a search field that runs on every keystroke. Keyed on mtime
# so a `--sync` in another process is picked up without a restart.
_CORPUS: tuple[float, list] | None = None


def _load_portal_corpus() -> tuple[list, str | None]:
    """Parse once and pre-tokenise. BM25 needs term counts and lengths, and
    re-tokenising 12k bodies per keystroke is the difference between a search
    that feels instant and one that does not."""
    global _CORPUS
    try:
        mtime = PORTAL_CORPUS_CACHE.stat().st_mtime
    except OSError:
        return [], None
    if _CORPUS is not None and _CORPUS[0] == mtime:
        return _CORPUS[1], None
    try:
        raw = json.loads(PORTAL_CORPUS_CACHE.read_text()).get("docs", [])
    except (OSError, ValueError) as exc:
        return [], "The saved search index is damaged. It will be downloaded again."

    docs = []
    for d in raw:
        if not isinstance(d, dict):
            continue
        body = tokenize(d.get("b", ""))
        head = tokenize(d.get("t", ""))
        counts: dict[str, int] = {}
        for tok in body:
            counts[tok] = counts.get(tok, 0) + 1
        # A title term is not one mention, it is what the document is about.
        # Folding it in as repeated occurrences lets BM25's saturation handle
        # it rather than bolting a separate additive bonus onto the side.
        for tok in head:
            counts[tok] = counts.get(tok, 0) + TITLE_REPEAT
        docs.append({"t": d.get("t", ""), "u": d.get("u", ""),
                     "c": d.get("c", ""), "b": d.get("b", ""),
                     "_c": counts, "_len": len(body) + len(head) * TITLE_REPEAT})
    _CORPUS = (mtime, docs)
    return docs, None


def search_portal_corpus(query: str, limit: int) -> tuple[list[Result], str | None]:
    """BM25 over the pre-tokenised corpus.

    Two passes, deliberately. The first counts how many documents contain each
    query term; the second scores. That is what makes IDF possible, and IDF is
    what separates results that all merely *contain* a common word — the
    previous scorer had none, despite a docstring claiming otherwise, so a
    query like "immigration paradise" returned a flat band of near-identical
    scores in which "Solitary confinement" sat above "Asylum seeker" for no
    reason a reader could defend.
    """
    status = portal_corpus_status()
    if not status["present"]:
        return [], None  # not an error; --sync has simply not been run

    docs, err = _load_portal_corpus()
    if err:
        return [], err
    tokens = query_terms(query)
    if not tokens or not docs:
        return [], None

    # Pass 1: document frequency, and the average length BM25 normalises by.
    n = len(docs)
    df = dict.fromkeys(tokens, 0)
    total_len = 0
    for d in docs:
        counts = d["_c"]
        total_len += d["_len"]
        for term in df:
            if term in counts:
                df[term] += 1
    avgdl = (total_len / n) or 1.0

    idf = {}
    for term, freq in df.items():
        # +0.5 smoothing, and max(…, 0.01) so a term in almost every document
        # contributes ~nothing instead of going negative and actively demoting
        # the documents that contain it.
        idf[term] = max(math.log(1.0 + (n - freq + 0.5) / (freq + 0.5)), 0.01)

    # Pass 2: score.
    out: list[Result] = []
    for d in docs:
        counts, dl = d["_c"], d["_len"] or 1
        score = 0.0
        matched = 0
        for term in tokens:
            tf = counts.get(term, 0)
            if not tf and len(term) >= 4:
                # Prefix, heavily discounted: "immigra" should reach
                # "immigration", but never outweigh a real hit.
                tf = 0.3 * sum(v for k, v in counts.items() if k.startswith(term))
            if tf <= 0:
                continue
            matched += 1
            denom = tf + BM25_K1 * (1 - BM25_B + BM25_B * dl / avgdl)
            score += idf[term] * (tf * (BM25_K1 + 1)) / denom
        if not matched:
            continue
        # Documents matching every term should clearly beat those matching one.
        score *= (matched / len(tokens)) ** 2
        title = d.get("t", "(untitled)")
        raw_url = (d.get("u") or "").strip()
        if raw_url.rstrip("/") in {p.rstrip("/") for p in NON_DOCUMENT_PATHS}:
            url = (f"https://{PORTAL_HOST}/tsearch/?q="
                   + urllib.parse.quote(title))
        else:
            url = normalise_url(raw_url)
        out.append(Result(
            title=title,
            url=url,
            snippet=(d.get("b") or "")[:200],
            source="portal",
            score=score,
            category=d.get("c", ""),
            matched=matched,
            terms=len(tokens),
        ))
    out.sort(key=lambda r: r.score, reverse=True)
    return prune(out)[:limit], None


def prune(results: list[Result]) -> list[Result]:
    """Drop the tail that is technically a match and obviously not an answer.

    Searching "dario amodei" returned Anthropic first — correct, the only
    document containing both names — followed by an Italian fencer, a museum,
    Doom, and the Turkish-Venetian wars. Each of those contains "dario"
    somewhere, so each is a real match, and each is noise. Showing them costs
    more than showing nothing: five wrong answers under one right one reads as
    a search engine that does not understand the question.

    Two cuts, in order:

    1. If ANY result matched every term of the query, keep only those. A
       document about both things you asked for beats one about half of them,
       and no amount of term frequency should change that.
    2. Otherwise drop anything scoring under a fifth of the best hit. This is
       what removes the flat tail on single-term queries without hard-coding
       a result count.
    """
    if not results:
        return results

    best = results[0]
    if best.terms > 1 and best.matched == best.terms:
        full = [r for r in results if r.matched == r.terms]
        if full:
            return full

    floor = results[0].score * 0.2
    return [r for r in results if r.score >= floor]


# --- built-in corpus --------------------------------------------------------

def _builtin_fields(doc: dict) -> tuple[str, str, str, str]:
    """Title, url, body, category from either seed shape.

    ISO / product install ships `search/seed.json` in portal short keys
    (`t`, `u`, `b`, `c`). Older hand-edited corpora used long keys
    (`title`, `url`, `text`/`body`). Accept both so a real seed scores
    instead of ranking every document as untitled empty text.
    """
    title = str(doc.get("title") or doc.get("t") or "(untitled)")
    url = str(doc.get("url") or doc.get("u") or "")
    body = str(
        doc.get("text") or doc.get("body") or doc.get("b") or ""
    )
    category = str(doc.get("category") or doc.get("c") or "")
    return title, url, body, category


def search_builtin(query: str, limit: int) -> tuple[list[Result], str | None]:
    """Search the offline help that ships with the image.

    Missing file is not an error — same posture as a portal corpus that has
    not been synced yet. Product install / ISO should ship the seed, but a
    partial guest must not paint every query with a warning row when Guide
    and web results still work. Corrupt JSON or unreadable-but-present file
    is still reported in plain language.
    """
    path = BUILTIN_CORPUS
    if not path.is_file():
        return [], None
    try:
        corpus = json.loads(path.read_text())
    except OSError:
        return [], "The built-in help is on this computer but couldn’t be opened."
    except ValueError:
        return [], "The built-in help file is damaged, so offline help is skipped."

    docs = corpus.get("docs", corpus if isinstance(corpus, list) else [])
    tokens = query_terms(query)
    out: list[Result] = []
    for doc in docs:
        if not isinstance(doc, dict):
            continue
        title, url, body, category = _builtin_fields(doc)
        text = f"{url} {body}".strip()
        score, matched = _score_matched(tokens, text, title)
        if score > 0:
            out.append(Result(
                title=title,
                url=url,
                snippet=body[:200],
                source="built-in",
                score=score,
                category=category,
                matched=matched,
                terms=len(tokens),
            ))
    out.sort(key=lambda r: r.score, reverse=True)
    return prune(out)[:limit], None


# --- workspace --------------------------------------------------------------

# Cheap filename-and-head match rather than a real index. A full-text index is
# the right answer eventually; shipping one badly is worse than shipping this
# and saying what it is.
SKIP_DIRS = {
    ".git", "node_modules", "__pycache__", ".cache", "target", ".venv",
    # Pseudo-filesystems. A home directory can easily contain one — a chroot
    # built under it has /proc, /sys and /dev inside, and walking those is at
    # best pointless and at worst a hang on a FIFO or a device node.
    "proc", "sys", "dev", "run",
    # Anything that is really a build tree rather than the user's work.
    "chroot", ".teddyos-vm", "teddyos-iso",
}
TEXT_SUFFIXES = {".md", ".txt", ".py", ".rs", ".js", ".ts", ".sh", ".json", ".toml", ".yaml", ".yml"}
MAX_FILES = 20000
MAX_BYTES = 4_000_000


def _walk(root: Path):
    """Yield files under root, stepping over what we may not read.

    `Path.rglob` is not usable here. It raises on the first unreadable
    directory and there is no way to resume, so a single root-owned path —
    /proc/1/cwd inside a build chroot, say — aborts the entire search with a
    PermissionError traceback instead of returning the results it already had.
    os.walk with onerror=ignore skips those and keeps going.
    """
    for dirpath, dirnames, filenames in os.walk(root, onerror=lambda _e: None):
        # Prune in place so os.walk does not descend at all, rather than
        # filtering afterwards — descending is what costs the time.
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        base = Path(dirpath)
        for name in filenames:
            yield base / name


def search_workspace(query: str, limit: int) -> tuple[list[Result], str | None]:
    roots = caps.workspace_paths()
    if not roots:
        return [], None

    tokens = query_terms(query)
    paths = path_terms(query)
    out: list[Result] = []
    seen = 0
    skipped = 0
    for root in roots:
        for path in _walk(Path(root)):
            if seen >= MAX_FILES:
                break
            if path.suffix.lower() not in TEXT_SUFFIXES:
                continue
            try:
                # Cheap guards before reading: a symlink into a device, a
                # socket, or a 2 GB log are all things a home directory has.
                stat = path.stat()
                if not S_ISREG(stat.st_mode) or stat.st_size > MAX_BYTES:
                    continue
                head = path.read_text(errors="ignore")[:4000]
            except OSError:
                # Unreadable is normal, not exceptional. Count it so the caller
                # can say so, and carry on.
                skipped += 1
                continue
            seen += 1
            # Title is the basename; the path string rides in the body so
            # parent folders ("iakovos/trading/…") participate in ranking.
            # Without that, a query for a project folder only hit files whose
            # *names* contained the words — never the tree they live in.
            path_text = str(path).replace("\\", "/").lower()
            score, matched = _score_matched(
                tokens, f"{head}\n{path_text}", path.name
            )
            # Strong boost when the query literally names a path segment of
            # this file — "iakovos/trading" against …/iakovos/trading/x.py.
            for term in paths:
                if term in path_text:
                    score = (score or 1.0) * 3.0
                    break
            if score > 0:
                out.append(Result(
                    title=path.name,
                    url=str(path),
                    snippet=head.strip().replace("\n", " ")[:200],
                    source="your files",
                    score=score,
                    matched=matched,
                    terms=len(tokens),
                ))
    out.sort(key=lambda r: r.score, reverse=True)
    note = f"{skipped} file(s) in your folders could not be read" if skipped else None
    return prune(out)[:limit], note


# --- the front door ---------------------------------------------------------

def search(query: str, limit: int = 10) -> Outcome:
    grants = caps.load()
    outcome = Outcome()
    paths = path_terms(query)
    workspace_hits = 0
    web_first = is_web_first_goal(query)
    outcome.web_first = web_first

    if grants.get("workspace.index"):
        results, err = search_workspace(query, limit)
        workspace_hits = len(results)
        outcome.results += results
        if err:
            outcome.errors.append(err)
    else:
        outcome.denied.append("Your files")

    if grants.get("portal.sync"):
        results, err, age = search_portal(query, limit)
        outcome.results += results
        outcome.portal_age = age
        if err:
            outcome.errors.append(err)

        # The portal proper, if it has been synced. Granting the network is
        # what makes this reachable at all — the index alone is a directory of
        # teddysearch's pages, this is the crawl behind them.
        corpus_results, corpus_err = search_portal_corpus(query, limit)
        outcome.results += corpus_results
        if corpus_err:
            outcome.errors.append(corpus_err)
        elif not portal_corpus_status()["present"]:
            # Only nag when the small index also came up empty. If Guide
            # already answered (GEX tools, market pages, …), a permanent
            # “still downloading” row is noise next to good hits — the same
            # defect class as “built-in help couldn’t be read” on a working
            # ticker result.
            if not results and not corpus_results:
                outcome.notes.append(
                    "Search is still downloading what it needs. Web results "
                    "will appear once that finishes.")
    else:
        outcome.denied.append("Online search")

    if grants.get("search.query"):
        results, err = search_builtin(query, limit)
        outcome.results += results
        if err:
            outcome.errors.append(err)
    else:
        outcome.denied.append("Built-in docs")

    # A path-shaped goal ("iakovos/trading") that never hit a local file is
    # almost always "the project is not on this machine", not "the web has no
    # page about those words". Say so above the noisy partial web hits.
    if paths and grants.get("workspace.index") and workspace_hits == 0:
        roots = caps.workspace_paths()
        where = ", ".join(str(r) for r in roots) if roots else "no folders yet"
        folders = where if where != "no folders yet" else "no folders chosen yet"
        outcome.notes.append(
            f"No files matching “{paths[0]}” in the folders you allowed "
            f"({folders}). Put the project in one of those folders, or open "
            "Permissions and add the folder under Your files.")

    outcome.results.sort(key=lambda r: r.score, reverse=True)
    # Web-first filter BEFORE the limit slice. Portal food noise scores higher
    # than offline Help cards; slicing first dropped Pizza Help in favor of
    # Focaccia, then the filter emptied the list.
    if web_first:
        outcome.results = _filter_web_first_results(outcome.results, query)
        # “how to make Ashure” may not BM25-hit the tip card (no shared rare
        # tokens). Still show the offline tip when Built-in help is on.
        if grants.get("search.query") and not any(
            "recipe" in (r.title or "").lower() or "near me" in (r.title or "").lower()
            for r in outcome.results
        ):
            outcome.results.insert(0, Result(
                title="Recipes delivery and near me",
                url="os://search/web-first",
                snippet=(
                    "Recipes, food delivery, and places near you are better "
                    "on the open web. Use Search the web first — the Guide "
                    "crawl is not a restaurant app."
                ),
                source="built-in",
                score=1.0,
                category="food",
                matched=0,
                terms=len(query_terms(query)) or 1,
            ))
        outcome.notes.append(
            "Recipes, delivery, and “near me” are better on the open web — "
            "use Search the web below (or first).")
    # Full-match prune across sources, not only inside each one — otherwise a
    # perfect local hit still sits under a pile of single-term web noise.
    outcome.results = prune(outcome.results)[:limit]
    return outcome
