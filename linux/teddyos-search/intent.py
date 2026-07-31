"""Local intent router for Search.

Turns whatever someone typed into a small structured job the product can run.
v1 is pure rules (instant, offline, unit-testable). A later step can refine
ambiguous cases with a signed-in AI — same Intent shape, no free-roaming agent.

Agent decides. Product executes. User always owns send.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from enum import Enum
from typing import Iterable

import search as engine


class IntentKind(str, Enum):
    """Product paths Search knows how to run."""

    PROJECT_HELP = "project_help"      # work on a folder / repo
    MESSAGE_REPLY = "message_reply"    # AI draft replies (never auto-send)
    FREEFORM_HELP = "freeform_help"    # everyday task → Get help at home
    LOOKUP = "lookup"                  # keyword / corpus / web
    SETUP = "setup"                    # sign-in / permissions (reserved)
    AMBIGUOUS = "ambiguous"            # offer plain choices; don’t guess


class Channel(str, Enum):
    LINKEDIN = "linkedin"
    WHATSAPP = "whatsapp"
    EMAIL = "email"
    UNKNOWN = "unknown"
    NONE = "none"


# Confidence floors for product behaviour.
CONFIDENT = 0.75
LIKELY = 0.55
GUESS = 0.40


@dataclass(frozen=True)
class Intent:
    """What Search should do with one query."""

    kind: IntentKind
    raw_query: str
    subject: str = ""
    channel: Channel = Channel.NONE
    confidence: float = 0.0
    # Human one-liner for the UI (never jargon).
    summary: str = ""
    # Alternate intents when kind is AMBIGUOUS (or always available as chips).
    alternatives: tuple["Intent", ...] = field(default_factory=tuple)
    # True when the product may auto-start the playbook (messaging drafts).
    auto_start_ok: bool = False
    source: str = "rules"  # rules | user_pick | model (future)

    def with_user_pick(self) -> "Intent":
        """User tapped a choice — treat as certain."""
        return replace(self, confidence=1.0, source="user_pick", alternatives=())


def classify(text: str) -> Intent:
    """Map free text → Intent (local rules only).

    Order matters: messaging and freeform before project slug, so
    “reply to my messages” never becomes a GitHub hunt.
    """
    raw = (text or "").strip()
    if not raw:
        return Intent(
            kind=IntentKind.LOOKUP,
            raw_query="",
            confidence=1.0,
            summary="Type what you need.",
        )

    low = raw.lower()
    subject = engine.focus_query(raw)

    # --- explicit messaging (high confidence) --------------------------------
    want_li = engine.is_linkedin_messages_goal(raw)
    want_wa = engine.is_whatsapp_messages_goal(raw)

    if want_li and want_wa:
        # Both named — ambiguous channel, clear job.
        primary_wa = low.rfind("whatsapp") > low.rfind("linkedin")
        primary = _message_intent(
            raw, subject,
            Channel.WHATSAPP if primary_wa else Channel.LINKEDIN,
            confidence=0.7,
            summary=(
                "Looks like you want AI help replying to messages. "
                "Pick LinkedIn or WhatsApp — nothing is sent without you."
            ),
        )
        other = _message_intent(
            raw, subject,
            Channel.LINKEDIN if primary_wa else Channel.WHATSAPP,
            confidence=0.7,
        )
        return replace(
            primary,
            kind=IntentKind.AMBIGUOUS,
            alternatives=(primary, other),
            auto_start_ok=False,
        )

    if want_li:
        return _message_intent(
            raw, subject, Channel.LINKEDIN, confidence=0.95,
            summary=(
                "Looks like you want AI help with LinkedIn messages. "
                "AI drafts; you always send."
            ),
            auto_start_ok=True,
        )

    if want_wa:
        return _message_intent(
            raw, subject, Channel.WHATSAPP, confidence=0.95,
            summary=(
                "Looks like you want AI help with WhatsApp messages. "
                "AI drafts; you always send."
            ),
            auto_start_ok=True,
        )

    # --- generic reply / inbox without channel → ambiguous messaging ---------
    if _looks_like_generic_reply(low):
        li = _message_intent(
            raw, subject, Channel.LINKEDIN, confidence=0.5,
            summary="Draft LinkedIn replies with AI (you paste and send).",
        )
        wa = _message_intent(
            raw, subject, Channel.WHATSAPP, confidence=0.5,
            summary="Draft WhatsApp replies with AI (you paste and send).",
        )
        lookup = Intent(
            kind=IntentKind.LOOKUP,
            raw_query=raw,
            subject=subject,
            confidence=0.35,
            summary="Just search for this instead.",
        )
        return Intent(
            kind=IntentKind.AMBIGUOUS,
            raw_query=raw,
            subject=subject,
            channel=Channel.UNKNOWN,
            confidence=0.55,
            summary=(
                "Looks like you want help replying to messages. "
                "Which inbox? AI only drafts — you always send."
            ),
            alternatives=(wa, li, lookup),
            auto_start_ok=False,
            source="rules",
        )

    # --- goals: freeform tasks vs project folders ----------------------------
    if engine.is_work_goal(raw):
        # Everyday tasks first — never a GitHub dead-end.
        if engine.is_freeform_help_goal(raw) and not _looks_like_project_slug(subject):
            return Intent(
                kind=IntentKind.FREEFORM_HELP,
                raw_query=raw,
                subject=subject,
                confidence=0.85,
                summary="We’ll help with that in plain words.",
                auto_start_ok=False,
            )
        if _looks_like_project_slug(subject) or _has_work_on_phrase(low):
            return Intent(
                kind=IntentKind.PROJECT_HELP,
                raw_query=raw,
                subject=subject,
                confidence=0.85 if _has_work_on_phrase(low) else 0.7,
                summary=(
                    f"Looks like you want help with “{subject}”."
                    if subject
                    else "Looks like you want help with a project."
                ),
            )
        # Work-shaped but unclear — let the person pick.
        proj = Intent(
            kind=IntentKind.PROJECT_HELP,
            raw_query=raw,
            subject=subject,
            confidence=0.5,
            summary=f"Work on a project called “{subject or 'this'}”.",
        )
        free = Intent(
            kind=IntentKind.FREEFORM_HELP,
            raw_query=raw,
            subject=subject,
            confidence=0.45,
            summary="Just get help with what you typed.",
        )
        return Intent(
            kind=IntentKind.AMBIGUOUS,
            raw_query=raw,
            subject=subject,
            confidence=0.5,
            summary="What do you want to do?",
            alternatives=(proj, free),
        )

    # --- plain lookup --------------------------------------------------------
    return Intent(
        kind=IntentKind.LOOKUP,
        raw_query=raw,
        subject=subject,
        confidence=0.9,
        summary="Searching for that.",
    )


def force_message_reply(raw: str, channel: Channel) -> Intent:
    """Build a user-picked messaging intent (choice row)."""
    subject = engine.focus_query(raw) if raw else ""
    return _message_intent(
        raw or f"reply to my {channel.value} messages",
        subject,
        channel,
        confidence=1.0,
        auto_start_ok=True,
        source="user_pick",
    ).with_user_pick()


def force_kind(raw: str, kind: IntentKind, *, subject: str = "") -> Intent:
    """User picked a non-messaging alternative."""
    subject = subject or engine.focus_query(raw)
    base = Intent(
        kind=kind,
        raw_query=raw,
        subject=subject,
        confidence=1.0,
        source="user_pick",
        summary=_summary_for_kind(kind, subject),
        auto_start_ok=kind == IntentKind.MESSAGE_REPLY,
    )
    return base


def choice_label(intent: Intent) -> str:
    """Short row title for ambiguous choices."""
    if intent.kind is IntentKind.MESSAGE_REPLY:
        if intent.channel is Channel.WHATSAPP:
            return "Reply on WhatsApp with AI"
        if intent.channel is Channel.LINKEDIN:
            return "Reply on LinkedIn with AI"
        return "Reply to messages with AI"
    if intent.kind is IntentKind.PROJECT_HELP:
        sub = intent.subject or "a project"
        return f"Work on “{sub}”"
    if intent.kind is IntentKind.FREEFORM_HELP:
        return "Get help with what I typed"
    if intent.kind is IntentKind.LOOKUP:
        return "Just search"
    if intent.kind is IntentKind.SETUP:
        return "Sign in / settings"
    return intent.summary or "Continue"


def choice_subtitle(intent: Intent) -> str:
    if intent.kind is IntentKind.MESSAGE_REPLY:
        return "AI drafts paste-ready replies — you always send"
    if intent.kind is IntentKind.PROJECT_HELP:
        return "Open tools for this project folder"
    if intent.kind is IntentKind.FREEFORM_HELP:
        return "Ask AI in plain words"
    if intent.kind is IntentKind.LOOKUP:
        return "Look it up in the guide and on the web"
    return intent.summary or ""


# --- internals ---------------------------------------------------------------


def _message_intent(
    raw: str,
    subject: str,
    channel: Channel,
    *,
    confidence: float,
    summary: str = "",
    auto_start_ok: bool = False,
    source: str = "rules",
) -> Intent:
    if not summary:
        name = {
            Channel.LINKEDIN: "LinkedIn",
            Channel.WHATSAPP: "WhatsApp",
            Channel.EMAIL: "email",
        }.get(channel, "messages")
        summary = f"AI help replying on {name} — you always send."
    return Intent(
        kind=IntentKind.MESSAGE_REPLY,
        raw_query=raw,
        subject=subject,
        channel=channel,
        confidence=confidence,
        summary=summary,
        auto_start_ok=auto_start_ok and confidence >= CONFIDENT,
        source=source,
    )


def _looks_like_generic_reply(low: str) -> bool:
    """Reply/inbox language without a named channel."""
    if "linkedin" in low or "whatsapp" in low or "whats app" in low:
        return False
    replyish = any(
        p in low
        for p in (
            "reply to my message",
            "reply to my messages",
            "respond to my message",
            "respond to my messages",
            "answer my message",
            "answer my messages",
            "clear my inbox",
            "check my messages",
            "check my inbox",
            "catch up on my messages",
            "help me reply",
            "help me respond",
            "draft a reply",
            "draft replies",
            "text people back",
            "reply with ai",
            "respond with ai",
        )
    )
    if replyish:
        return True
    # Loose: reply/respond + message/inbox + optional ai
    has_verb = any(v in low for v in ("reply", "respond", "answer", "draft"))
    has_obj = any(o in low for o in ("message", "messages", "inbox", "dm", "dms"))
    return has_verb and has_obj


def _looks_like_project_slug(subject: str) -> bool:
    s = (subject or "").strip()
    if not s:
        return False
    low = s.lower()
    # Multi-word task language is not a folder name.
    task_bits = (
        "message", "messages", "email", "inbox", "reply", "linkedin",
        "whatsapp", "meeting", "calendar", "help me",
    )
    if any(b in low for b in task_bits):
        return False
    words = low.split()
    if len(words) >= 4:
        return False
    # owner/repo or single slug
    if "/" in s or "-" in s or "_" in s:
        return True
    if len(words) == 1 and len(words[0]) >= 2:
        return True
    if len(words) == 2 and all(len(w) >= 2 for w in words):
        # "my app" weak; "iakovos trading" ok-ish
        if words[0] in ("my", "the", "a", "an"):
            return False
        return True
    return False


def _has_work_on_phrase(low: str) -> bool:
    return any(
        p in low
        for p in (
            "work on ", "working on ", "wanna work", "want to work",
            "open project", "continue on ",
        )
    )


def _summary_for_kind(kind: IntentKind, subject: str) -> str:
    if kind is IntentKind.PROJECT_HELP:
        return f"Help with “{subject or 'this project'}”."
    if kind is IntentKind.FREEFORM_HELP:
        return "Help with what you typed."
    if kind is IntentKind.LOOKUP:
        return "Search for that."
    if kind is IntentKind.MESSAGE_REPLY:
        return "AI help replying — you always send."
    return ""


def intents_equal_job(a: Intent, b: Intent) -> bool:
    return a.kind == b.kind and a.channel == b.channel


def all_choice_intents(intent: Intent) -> Iterable[Intent]:
    """Flatten primary + alternatives for UI rows."""
    if intent.kind is IntentKind.AMBIGUOUS:
        seen: set[tuple[str, str]] = set()
        for alt in intent.alternatives:
            key = (alt.kind.value, alt.channel.value)
            if key in seen:
                continue
            seen.add(key)
            yield alt
        return
    yield intent
