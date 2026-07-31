"""Gamification for teddyOS — personas, levels, and mode switches.

Progress tracks *fluency* overall and a **level per persona** (Ultracode,
LinkedIn, Fitness, …). Each ask detects an audience:

  • awards XP to that persona (level up: “I’m level 2 in LinkedIn”)
  • if the persona changed since last ask → switch event + celebration
  • Ultracode (code) stays the flashiest path

Never blocks; never shows quest IDs.
"""

from __future__ import annotations

import json
import os
import time
from dataclasses import dataclass, field
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any


CONFIG_DIR = Path(
    os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")
) / "teddyos"
PROGRESS_FILE = CONFIG_DIR / "progress.json"

# Overall journey titles (global fluency XP).
_LEVELS: list[tuple[int, str, str]] = [
    (0, "Just getting started", "Ask anything — plain words are perfect."),
    (15, "Curious", "You’re asking real questions. Mode follows how you talk."),
    (40, "Explorer", "You know the flow. Try a new kind of ask to unlock modes."),
    (80, "Builder", "You’re steering the helpers. Personas switch with your ask."),
    (140, "Ultracode Pilot", "You talk code — we switch modes and dig in."),
    (220, "Multi-AI Pro", "You fan out to several AIs and compare answers."),
    (320, "Ultracode Regular", "Code-mode is how you work here."),
    (450, "Fluency+", "You and the tools finish each other’s sentences."),
]

# Per-persona ladder: min_xp → display level number + title.
# User-facing: “Level 2 · Apprentice” in that mode.
_PERSONA_LADDER: list[tuple[int, int, str]] = [
    (0, 1, "Novice"),
    (12, 2, "Apprentice"),
    (30, 3, "Regular"),
    (60, 4, "Pro"),
    (100, 5, "Master"),
    (160, 6, "Legend"),
    (240, 7, "Virtuoso"),
]

# id → (title, blurb, xp_to_fluency)
_BADGES: dict[str, tuple[str, str, int]] = {
    "first_ask": (
        "First question",
        "You asked for help. That’s the whole product.",
        12,
    ),
    "ultracode": (
        "Ultracode unlocked",
        "Your words sounded like code — we switched modes for you.",
        30,
    ),
    "ultracode_3": (
        "Ultracode ×3",
        "Three code-mode asks. You’re getting fluent.",
        25,
    ),
    "ultracode_10": (
        "Ultracode fluent",
        "Ten Ultracode sessions. The helpers know you speak code.",
        45,
    ),
    "plain_then_code": (
        "Found the switch",
        "You asked simply, then later in code — both modes work.",
        20,
    ),
    "mode_switcher": (
        "Mode switcher",
        "You changed personas mid-session — the chip follows you.",
        16,
    ),
    "mode_switcher_5": (
        "Shape-shifter",
        "Five persona switches. You’re fluent across modes.",
        24,
    ),
    "multi_persona_3": (
        "Three hats",
        "Leveled up in three different personas.",
        20,
    ),
    "multi_ai": (
        "Crowd of helpers",
        "You asked more than one AI at once.",
        18,
    ),
    "across_all": (
        "Across all answers",
        "We compared every AI and pulled one clear takeaway.",
        22,
    ),
    "ultracode_multi": (
        "Ultracode + multi-AI",
        "Code-mode ask across several helpers. High skill play.",
        28,
    ),
    "first_signin": (
        "Signed in",
        "A helper is connected. Setup, not the main game.",
        6,
    ),
    "all_set": (
        "All set up",
        "Important sign-ins done — now the real asks matter.",
        10,
    ),
    "first_clone": (
        "Project downloaded",
        "A GitHub folder is on this computer. Ready to ask about it.",
        8,
    ),
    "tour_done": (
        "Showed around",
        "Tour done. The game is in Search → Get help.",
        4,
    ),
    "streak_3": (
        "Three-day streak",
        "You came back three days in a row.",
        15,
    ),
    "streak_7": (
        "Week streak",
        "Seven days of showing up.",
        30,
    ),
}

_ACTION_XP: dict[str, int] = {
    "plain_ask": 4,
    "persona_ask": 8,       # specialist persona (LinkedIn, Fitness, …)
    "ultracode_ask": 14,
    "multi_ai_bonus": 5,
    "ultracode_multi_bonus": 6,
    "switch_bonus": 3,      # small spice for changing modes
    "synthesis": 8,
    "signin": 3,
    "clone": 4,
    "tour": 2,
    "daily": 2,
}


@dataclass
class ProgressEvent:
    """Something the UI can celebrate (toast / animation)."""
    kind: str  # xp | badge | level | streak | ultracode | switch | persona_level
    title: str
    detail: str = ""
    xp_delta: int = 0
    badge_id: str = ""
    level_title: str = ""
    ultracode: bool = False
    persona: str = ""
    persona_level: int = 0
    switched_from: str = ""


@dataclass
class ProgressSnapshot:
    xp: int = 0
    level: int = 0
    level_title: str = "Just getting started"
    level_blurb: str = ""
    next_level_xp: int | None = None
    xp_into_level: int = 0
    xp_for_level: int = 25
    streak_days: int = 0
    badges: list[str] = field(default_factory=list)
    badge_titles: list[str] = field(default_factory=list)
    ask_count: int = 0
    plain_asks: int = 0
    ultracode_asks: int = 0
    ultracode_ratio: float = 0.0
    # Active / last persona skill
    active_persona: str = "plain"
    active_persona_level: int = 1
    active_persona_label: str = "Keeping it simple"
    persona_levels: dict[str, int] = field(default_factory=dict)  # id → level #
    persona_xp: dict[str, int] = field(default_factory=dict)
    top_personas: list[tuple[str, int, str]] = field(default_factory=list)
    # (id, level, short label)
    last_switch: str = ""
    switch_count: int = 0
    events: list[ProgressEvent] = field(default_factory=list)


def _today() -> str:
    return date.today().isoformat()


def _load_raw() -> dict[str, Any]:
    try:
        data = json.loads(PROGRESS_FILE.read_text())
        if isinstance(data, dict):
            return data
    except (OSError, json.JSONDecodeError, TypeError):
        pass
    return {}


def _save_raw(data: dict[str, Any]) -> None:
    try:
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        tmp = PROGRESS_FILE.with_suffix(".tmp")
        tmp.write_text(json.dumps(data, indent=2) + "\n")
        tmp.replace(PROGRESS_FILE)
    except OSError:
        pass


def _level_for_xp(xp: int) -> tuple[int, str, str, int | None]:
    idx = 0
    for i, (need, title, blurb) in enumerate(_LEVELS):
        if xp >= need:
            idx = i
        else:
            break
    _need, title, blurb = _LEVELS[idx]
    nxt = _LEVELS[idx + 1][0] if idx + 1 < len(_LEVELS) else None
    return idx, title, blurb, nxt


def _persona_level_for_xp(xp: int) -> tuple[int, str, int | None]:
    """Return (level_number, title, xp_for_next or None)."""
    lvl, title = 1, "Novice"
    nxt: int | None = _PERSONA_LADDER[1][0] if len(_PERSONA_LADDER) > 1 else None
    for i, (need, num, name) in enumerate(_PERSONA_LADDER):
        if xp >= need:
            lvl, title = num, name
            nxt = (
                _PERSONA_LADDER[i + 1][0]
                if i + 1 < len(_PERSONA_LADDER)
                else None
            )
        else:
            break
    return lvl, title, nxt


def _persona_bucket(data: dict[str, Any]) -> dict[str, Any]:
    raw = data.get("personas")
    if not isinstance(raw, dict):
        raw = {}
        data["personas"] = raw
    return raw


def _persona_stats(data: dict[str, Any], pid: str) -> dict[str, int]:
    bucket = _persona_bucket(data)
    entry = bucket.get(pid)
    if not isinstance(entry, dict):
        entry = {"xp": 0, "asks": 0}
        bucket[pid] = entry
    entry["xp"] = int(entry.get("xp") or 0)
    entry["asks"] = int(entry.get("asks") or 0)
    return entry  # type: ignore[return-value]


def _chip_label_for(pid: str) -> str:
    if pid == "code":
        return "Ultracode"
    if pid == "plain":
        return "Simple"
    try:
        from audience import Audience, audience_chip_label  # type: ignore
        for a in Audience:
            if a.value == pid:
                return audience_chip_label(a).replace(" mode", "").replace("-aware", "")
    except Exception:  # noqa: BLE001
        pass
    return pid.replace("_", " ").title()


def _short_label(pid: str) -> str:
    if pid == "code":
        return "Ultracode"
    if pid == "plain":
        return "Simple"
    lab = _chip_label_for(pid)
    # Keep strip short
    return lab[:18]


def snapshot(events: list[ProgressEvent] | None = None) -> ProgressSnapshot:
    data = _load_raw()
    xp = int(data.get("xp") or 0)
    idx, title, blurb, nxt = _level_for_xp(xp)
    floor = _LEVELS[idx][0]
    span = (nxt - floor) if nxt is not None else max(xp - floor, 1)
    badges = list(data.get("badges") or [])
    titles = [_BADGES[b][0] for b in badges if b in _BADGES]
    plain_n = int(data.get("plain_asks") or 0)
    ultra_n = int(data.get("ultracode_asks") or 0)
    total_asks = plain_n + ultra_n
    # Include other persona asks in ratio denom when present
    persona_asks_total = sum(
        int(v.get("asks") or 0)
        for v in (_persona_bucket(data).values())
        if isinstance(v, dict)
    )
    denom = max(1, persona_asks_total or total_asks) if (persona_asks_total or total_asks) else 1
    ratio = (ultra_n / denom) if denom else 0.0

    active = (data.get("last_audience") or "plain").strip() or "plain"
    pstats = _persona_stats(data, active)
    p_lvl, p_title, _ = _persona_level_for_xp(int(pstats.get("xp") or 0))

    levels: dict[str, int] = {}
    pxp: dict[str, int] = {}
    ranked: list[tuple[str, int, int, str]] = []  # id, level, xp, label
    for pid, entry in _persona_bucket(data).items():
        if not isinstance(entry, dict):
            continue
        px = int(entry.get("xp") or 0)
        if px <= 0 and int(entry.get("asks") or 0) <= 0:
            continue
        lv, _t, _n = _persona_level_for_xp(px)
        levels[pid] = lv
        pxp[pid] = px
        ranked.append((pid, lv, px, _short_label(pid)))
    ranked.sort(key=lambda t: (-t[1], -t[2], t[0]))
    top = [(pid, lv, lab) for pid, lv, _px, lab in ranked[:5]]

    return ProgressSnapshot(
        xp=xp,
        level=idx,
        level_title=title,
        level_blurb=blurb,
        next_level_xp=nxt,
        xp_into_level=max(0, xp - floor),
        xp_for_level=max(1, span),
        streak_days=int(data.get("streak_days") or 0),
        badges=badges,
        badge_titles=titles,
        ask_count=int(data.get("ask_count") or 0),
        plain_asks=plain_n,
        ultracode_asks=ultra_n,
        ultracode_ratio=ratio,
        active_persona=active,
        active_persona_level=p_lvl,
        active_persona_label=f"Level {p_lvl} · {_short_label(active)}",
        persona_levels=levels,
        persona_xp=pxp,
        top_personas=top,
        last_switch=str(data.get("last_switch_msg") or ""),
        switch_count=int(data.get("switch_count") or 0),
        events=list(events or []),
    )


def _touch_streak(data: dict[str, Any], events: list[ProgressEvent]) -> None:
    today = _today()
    last = (data.get("last_active") or "").strip()
    streak = int(data.get("streak_days") or 0)
    if last == today:
        return
    if last:
        try:
            prev = date.fromisoformat(last)
            delta = (date.today() - prev).days
        except ValueError:
            delta = 999
        if delta == 1:
            streak += 1
        elif delta > 1:
            streak = 1
        else:
            streak = max(streak, 1)
    else:
        streak = 1
    data["last_active"] = today
    data["streak_days"] = streak

    daily_key = data.get("daily_xp_day") or ""
    if daily_key != today:
        data["daily_xp_day"] = today
        gain = _ACTION_XP["daily"]
        data["xp"] = int(data.get("xp") or 0) + gain
        events.append(ProgressEvent(
            kind="xp",
            title=f"+{gain} today",
            detail="Showing up keeps your fluency streak warm.",
            xp_delta=gain,
        ))

    if streak >= 7 and "streak_7" not in (data.get("badges") or []):
        _award_badge(data, "streak_7", events)
    elif streak >= 3 and "streak_3" not in (data.get("badges") or []):
        _award_badge(data, "streak_3", events)


def _award_badge(
    data: dict[str, Any], badge_id: str, events: list[ProgressEvent],
) -> bool:
    if badge_id not in _BADGES:
        return False
    badges = list(data.get("badges") or [])
    if badge_id in badges:
        return False
    title, blurb, xp = _BADGES[badge_id]
    badges.append(badge_id)
    data["badges"] = badges
    data["xp"] = int(data.get("xp") or 0) + int(xp)
    events.append(ProgressEvent(
        kind="badge",
        title=title,
        detail=blurb,
        xp_delta=xp,
        badge_id=badge_id,
        ultracode=badge_id.startswith("ultracode") or badge_id == "plain_then_code",
    ))
    return True


def _check_level_up(
    before_xp: int, after_xp: int, events: list[ProgressEvent],
) -> None:
    b_idx, _, _, _ = _level_for_xp(before_xp)
    a_idx, title, blurb, _ = _level_for_xp(after_xp)
    if a_idx > b_idx:
        events.append(ProgressEvent(
            kind="level",
            title=f"Level up · {title}",
            detail=blurb,
            level_title=title,
            ultracode="Ultracode" in title,
        ))


def _award_persona_xp(
    data: dict[str, Any],
    pid: str,
    gain: int,
    events: list[ProgressEvent],
    *,
    ultracode: bool,
) -> None:
    """Add XP to one persona skill; emit persona_level events on level-up."""
    if gain <= 0:
        return
    stats = _persona_stats(data, pid)
    before = int(stats["xp"])
    after = before + gain
    stats["xp"] = after
    stats["asks"] = int(stats["asks"]) + 1
    b_lvl, _bt, _ = _persona_level_for_xp(before)
    a_lvl, a_title, _ = _persona_level_for_xp(after)
    label = _short_label(pid)
    if a_lvl > b_lvl:
        events.append(ProgressEvent(
            kind="persona_level",
            title=f"Level {a_lvl} · {label}",
            detail=f"You’re a {a_title} in {label}.",
            persona=pid,
            persona_level=a_lvl,
            level_title=a_title,
            ultracode=ultracode,
        ))

    # Badge: three distinct personas with at least level 2
    leveled = 0
    for other, entry in _persona_bucket(data).items():
        if not isinstance(entry, dict):
            continue
        lv, _, _ = _persona_level_for_xp(int(entry.get("xp") or 0))
        if lv >= 2:
            leveled += 1
    if leveled >= 3:
        _award_badge(data, "multi_persona_3", events)


def award(
    action: str = "",
    *,
    badge: str = "",
    ultracode: bool = False,
    multi_ai: bool = False,
    ask: bool = False,
    persona: str = "",
    switched_from: str = "",
) -> ProgressSnapshot:
    """Record a product moment. Persona asks + Ultracode drive XP."""
    data = _load_raw()
    events: list[ProgressEvent] = []
    before = int(data.get("xp") or 0)
    pid = (persona or ("code" if ultracode else "plain")).strip() or "plain"

    _touch_streak(data, events)

    if ask:
        data["ask_count"] = int(data.get("ask_count") or 0) + 1
        data["last_ask_ts"] = time.time()
        prev = (data.get("last_audience") or "").strip()
        data["last_audience"] = pid

        if ultracode or pid == "code":
            data["ultracode_asks"] = int(data.get("ultracode_asks") or 0) + 1
            data["ever_ultracode"] = True
            gain = _ACTION_XP["ultracode_ask"]
            detail = "Ultracode — code-mode help"
            if multi_ai:
                gain += _ACTION_XP["ultracode_multi_bonus"]
                detail = "Ultracode + multi-AI"
            kind = "ultracode"
            title = f"+{gain} Ultracode"
        elif pid == "plain":
            data["plain_asks"] = int(data.get("plain_asks") or 0) + 1
            data["ever_plain"] = True
            gain = _ACTION_XP["plain_ask"]
            detail = "Simple mode — plain language"
            if multi_ai:
                gain += _ACTION_XP["multi_ai_bonus"]
                detail = "Multi-AI (simple mode)"
            kind = "xp"
            title = f"+{gain} for asking"
        else:
            # Specialist persona
            data["ever_plain"] = data.get("ever_plain") or True
            gain = _ACTION_XP["persona_ask"]
            label = _short_label(pid)
            detail = f"{label} mode"
            if multi_ai:
                gain += _ACTION_XP["multi_ai_bonus"]
                detail = f"{label} + multi-AI"
            kind = "xp"
            title = f"+{gain} {label}"

        # Mode switch: previous ask was a different persona
        switched = bool(prev and prev != pid)
        if switched:
            gain += _ACTION_XP["switch_bonus"]
            data["switch_count"] = int(data.get("switch_count") or 0) + 1
            from_lab = _short_label(prev)
            to_lab = _short_label(pid)
            msg = f"{from_lab} → {to_lab}"
            data["last_switch_msg"] = msg
            events.append(ProgressEvent(
                kind="switch",
                title=f"Switched · {to_lab}",
                detail=f"Mode change: {msg}",
                xp_delta=_ACTION_XP["switch_bonus"],
                persona=pid,
                switched_from=prev,
                ultracode=(pid == "code"),
            ))
            _award_badge(data, "mode_switcher", events)
            if int(data.get("switch_count") or 0) >= 5:
                _award_badge(data, "mode_switcher_5", events)
            if prev == "plain" and pid == "code":
                _award_badge(data, "plain_then_code", events)
            title = f"+{gain} · switch to {to_lab}"

        data["xp"] = int(data.get("xp") or 0) + gain
        events.append(ProgressEvent(
            kind=kind,
            title=title,
            detail=detail,
            xp_delta=gain,
            ultracode=(pid == "code"),
            persona=pid,
            switched_from=prev if switched else "",
        ))

        _award_persona_xp(
            data, pid, gain, events, ultracode=(pid == "code"),
        )

        _award_badge(data, "first_ask", events)
        if pid == "code":
            _award_badge(data, "ultracode", events)
            ultra_n = int(data.get("ultracode_asks") or 0)
            if ultra_n >= 3:
                _award_badge(data, "ultracode_3", events)
            if ultra_n >= 10:
                _award_badge(data, "ultracode_10", events)
            if multi_ai:
                _award_badge(data, "ultracode_multi", events)
            if data.get("ever_plain") and not switched:
                # first time code after plain without this-session switch
                if prev == "plain":
                    _award_badge(data, "plain_then_code", events)
        if multi_ai:
            _award_badge(data, "multi_ai", events)

    if badge:
        _award_badge(data, badge, events)

    if action and action in _ACTION_XP and not ask:
        if action not in (
            "plain_ask", "persona_ask", "ultracode_ask", "multi_ai_bonus",
            "ultracode_multi_bonus", "switch_bonus", "daily",
        ):
            gain = _ACTION_XP[action]
            data["xp"] = int(data.get("xp") or 0) + gain
            events.append(ProgressEvent(
                kind="xp",
                title=f"+{gain}",
                detail=action.replace("_", " "),
                xp_delta=gain,
            ))

    after = int(data.get("xp") or 0)
    _check_level_up(before, after, events)
    data["updated_at"] = datetime.now(timezone.utc).isoformat()
    _save_raw(data)
    return snapshot(events)


def award_signin() -> ProgressSnapshot:
    return award(action="signin", badge="first_signin")


def award_all_set() -> ProgressSnapshot:
    return award(badge="all_set")


def award_clone() -> ProgressSnapshot:
    return award(action="clone", badge="first_clone")


def award_tour() -> ProgressSnapshot:
    return award(action="tour", badge="tour_done")


def award_ask(
    *,
    ultracode: bool = False,
    multi_ai: bool = False,
    persona: str = "",
) -> ProgressSnapshot:
    """Award an ask. Debounces XP if Search + Answers both fire."""
    data = _load_raw()
    now = time.time()
    last = float(data.get("last_ask_ts") or 0)
    pid = (persona or ("code" if ultracode else "plain")).strip() or "plain"
    if now - last < 12:
        events: list[ProgressEvent] = []
        # Still record a soft switch if audience changed during debounce
        prev = (data.get("last_audience") or "").strip()
        if prev and prev != pid:
            data["last_audience"] = pid
            data["switch_count"] = int(data.get("switch_count") or 0) + 1
            data["last_switch_msg"] = f"{_short_label(prev)} → {_short_label(pid)}"
            events.append(ProgressEvent(
                kind="switch",
                title=f"Switched · {_short_label(pid)}",
                detail=data["last_switch_msg"],
                persona=pid,
                switched_from=prev,
                ultracode=(pid == "code"),
            ))
            _award_badge(data, "mode_switcher", events)
        if ultracode or pid == "code":
            data["ever_ultracode"] = True
            data["last_audience"] = "code"
            _award_badge(data, "ultracode", events)
            if multi_ai:
                _award_badge(data, "ultracode_multi", events)
        else:
            data["last_audience"] = pid
        if multi_ai:
            _award_badge(data, "multi_ai", events)
        if events:
            _save_raw(data)
        return snapshot(events)
    return award(
        ask=True, ultracode=ultracode or pid == "code",
        multi_ai=multi_ai, persona=pid,
    )


def award_from_prompt(
    prompt: str,
    *,
    multi_ai: bool = False,
    tool_ids: list[str] | None = None,
) -> ProgressSnapshot:
    """Detect audience → award persona XP + switch when the mode changes."""
    pid = "plain"
    is_code = False
    try:
        from audience import Audience, detect_audience  # type: ignore
        aud = detect_audience(prompt or "")
        is_code = aud is Audience.CODE
        pid = getattr(aud, "value", None) or "plain"
    except Exception:  # noqa: BLE001
        is_code = False
        pid = "plain"
    if tool_ids is not None:
        multi_ai = len([t for t in tool_ids if t]) >= 2
    return award_ask(ultracode=is_code, multi_ai=multi_ai, persona=pid)


def award_synthesis() -> ProgressSnapshot:
    return award(action="synthesis", badge="across_all")


def format_toast(events: list[ProgressEvent]) -> str | None:
    if not events:
        return None
    order = {
        "persona_level": 0,
        "level": 1,
        "switch": 2,
        "badge": 3,
        "ultracode": 4,
        "streak": 5,
        "xp": 6,
    }
    best = sorted(events, key=lambda e: order.get(e.kind, 9))[0]
    if best.kind == "persona_level":
        return f"⬆ {best.title}"
    if best.kind == "level":
        return f"⬆ {best.title}"
    if best.kind == "switch":
        return f"⇄ {best.title}"
    if best.kind == "badge":
        star = "⚡" if best.ultracode else "★"
        return f"{star} {best.title}" + (f" · +{best.xp_delta}" if best.xp_delta else "")
    if best.kind == "ultracode":
        return f"⚡ {best.title}"
    if best.kind == "streak":
        return f"🔥 {best.title}"
    if best.xp_delta:
        return best.title
    return best.title


def progress_line(snap: ProgressSnapshot | None = None) -> str:
    """Status strip: overall title + top persona levels."""
    s = snap or snapshot()
    if s.ask_count <= 0 and s.ultracode_asks <= 0 and not s.persona_levels:
        return f"{s.level_title} · ask to start · modes switch with your ask"

    if s.next_level_xp is None:
        base = f"{s.level_title} · {s.xp} fluency"
    else:
        left = max(0, s.next_level_xp - s.xp)
        base = f"{s.level_title} · {s.xp} fluency · {left} to next"

    # Active persona level (what you just used / last used)
    if s.active_persona and s.active_persona_level:
        lab = _short_label(s.active_persona)
        base += f" · {lab} Lv.{s.active_persona_level}"

    # Up to two other top personas
    extras = [
        f"{lab} Lv.{lv}"
        for pid, lv, lab in s.top_personas
        if pid != s.active_persona
    ][:2]
    if extras:
        base += " · " + " · ".join(extras)

    if s.streak_days > 1:
        base += f" · 🔥{s.streak_days}"
    return base


def persona_level_line(persona: str | object | None = None) -> str:
    """One-liner for chips: 'Level 2 · LinkedIn'."""
    pid = "plain"
    if persona is None:
        pid = (_load_raw().get("last_audience") or "plain")
    elif hasattr(persona, "value"):
        pid = str(getattr(persona, "value") or "plain")
    else:
        pid = str(persona or "plain")
    data = _load_raw()
    stats = _persona_stats(data, pid)
    lv, title, _ = _persona_level_for_xp(int(stats.get("xp") or 0))
    return f"Level {lv} · {_short_label(pid)} · {title}"


def ultracode_meter(snap: ProgressSnapshot | None = None) -> tuple[float, str]:
    """(0–1 fraction for progress bar, caption). Fluency toward next level."""
    s = snap or snapshot()
    if s.next_level_xp is None:
        return 1.0, s.level_title
    frac = min(1.0, max(0.0, s.xp_into_level / max(1, s.xp_for_level)))
    return frac, progress_line(s)


def persona_meter(
    persona: str | object | None = None,
    snap: ProgressSnapshot | None = None,
) -> tuple[float, str]:
    """Progress within the active (or given) persona level."""
    pid = "plain"
    if persona is None:
        s = snap or snapshot()
        pid = s.active_persona or "plain"
    elif hasattr(persona, "value"):
        pid = str(getattr(persona, "value") or "plain")
    else:
        pid = str(persona or "plain")
    data = _load_raw()
    stats = _persona_stats(data, pid)
    xp = int(stats.get("xp") or 0)
    lv, title, nxt = _persona_level_for_xp(xp)
    # floor xp for this level
    floor = 0
    for need, num, _name in _PERSONA_LADDER:
        if num == lv:
            floor = need
            break
    if nxt is None:
        return 1.0, f"Level {lv} · {_short_label(pid)} · {title}"
    span = max(1, nxt - floor)
    frac = min(1.0, max(0.0, (xp - floor) / span))
    return frac, f"Level {lv} · {_short_label(pid)} · {title}"
