"""AI tools for 'work on …' goals: discovery, credits, and recent use.

Search lists every AI tool that is actually installed, checks remaining
credits where we can, and marks tools the person opened recently so the
next "work on …" does not bury Claude under Files.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


# How long "Recently used" stays true after a launch. Thirty days is long
# enough that a weekly project still shows the badge, short enough that a
# tool abandoned last quarter does not.
RECENT_DAYS = 30
# Cap the recents list so a noisy machine cannot grow the file without bound.
RECENT_MAX = 24

CONFIG_DIR = Path(
    os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")
) / "teddyos"
RECENTS_FILE = CONFIG_DIR / "work-tools.json"


@dataclass(frozen=True)
class WorkTool:
    """One way to open a project from a 'work on …' goal."""
    id: str
    title: str
    subtitle: str
    icon: str
    # argv; "{path}" is replaced with the project directory.
    argv: tuple[str, ...]
    # True for tools that spend a metered account (Claude, Cursor, …).
    metered: bool = False
    # AI / coding agents, as opposed to Files/Terminal helpers.
    is_ai: bool = True


@dataclass(frozen=True)
class CreditStatus:
    """What we know about remaining capacity for one tool.

    ok:
      True  — signed in and has remaining capacity
      False — depleted, not signed in, or missing
      None  — installed but we could not tell
    """
    ok: bool | None
    label: str


# Catalog of tools we know how to launch. Order is the default preference when
# nothing has been used yet (Claude first). Only entries whose binary exists
# on PATH are offered — an empty Cursor row on a machine that never shipped
# Cursor is worse than a short list.
#
# Each entry: id, title, subtitle, icon, metered, is_ai, candidate binaries
# (first hit wins), argv template after the binary (use "{path}" for the
# project directory; empty means [binary, path]).
_CATALOG: list[tuple] = [
    (
        "claude", "Claude", "Open this project in Claude Code",
        "teddyos-claude", True, True,
        ("teddyos-claude", "claude"),
        ("{path}",),
    ),
    (
        "cursor", "Cursor", "Open this project in Cursor",
        "text-editor", True, True,
        ("cursor", "cursor-agent"),
        ("{path}",),
    ),
    (
        "windsurf", "Windsurf", "Open this project in Windsurf",
        "text-editor", True, True,
        ("windsurf",),
        ("{path}",),
    ),
    (
        "codex", "Codex", "Open this project with OpenAI Codex",
        "text-editor", True, True,
        ("codex",),
        ("{path}",),
    ),
    (
        "gemini", "Gemini", "Open this project with Gemini CLI",
        "text-editor", True, True,
        ("gemini",),
        ("{path}",),
    ),
    (
        "aider", "Aider", "Pair-program on this project with Aider",
        "utilities-terminal-symbolic", True, True,
        ("aider",),
        ("{path}",),
    ),
    (
        "amp", "Amp", "Open this project with Amp",
        "text-editor", True, True,
        ("amp",),
        ("{path}",),
    ),
    (
        "crush", "Crush", "Open this project with Crush",
        "utilities-terminal-symbolic", True, True,
        ("crush",),
        ("{path}",),
    ),
    (
        "goose", "Goose", "Open this project with Goose",
        "utilities-terminal-symbolic", True, True,
        ("goose",),
        ("{path}",),
    ),
    (
        "ollama", "Ollama", "Chat with a local model in this project",
        "utilities-terminal-symbolic", False, True,
        ("ollama",),
        # Interactive run; cwd is set by the launcher.
        (),
    ),
    (
        "code", "VS Code", "Open this project in VS Code",
        "text-editor", False, True,
        ("code", "code-insiders"),
        ("{path}",),
    ),
    (
        "codium", "VSCodium", "Open this project in VSCodium",
        "text-editor", False, True,
        ("codium",),
        ("{path}",),
    ),
    (
        "zed", "Zed", "Open this project in Zed",
        "text-editor", False, True,
        ("zed",),
        ("{path}",),
    ),
]


def available_work_tools() -> list[WorkTool]:
    """Installed tools, recently used AI first, then the rest of the catalog.

    Files and Terminal always trail — they are escapes, not the point of a
    "work on …" goal.
    """
    found: list[WorkTool] = []
    seen_ids: set[str] = set()

    for entry in _CATALOG:
        tid, title, subtitle, icon, metered, is_ai, binaries, args = entry
        exe = _first_which(binaries)
        if not exe:
            continue
        argv = (exe,) + tuple(args)
        found.append(WorkTool(
            id=tid,
            title=title,
            subtitle=subtitle,
            icon=icon,
            argv=argv,
            metered=metered,
            is_ai=is_ai,
        ))
        seen_ids.add(tid)

    # Helpers — always available when the binary exists.
    file_mgr = (
        shutil.which("nautilus")
        or shutil.which("xdg-open")
        or "xdg-open"
    )
    helpers: list[WorkTool] = [
        WorkTool(
            id="files",
            title="Files",
            subtitle="Open the project folder",
            icon="folder",
            argv=(file_mgr, "{path}"),
            metered=False,
            is_ai=False,
        ),
    ]
    if shutil.which("gnome-terminal"):
        helpers.append(WorkTool(
            id="terminal",
            title="Terminal",
            subtitle="Open a command window in this project",
            icon="utilities-terminal-symbolic",
            argv=("gnome-terminal", "--working-directory={path}"),
            metered=False,
            is_ai=False,
        ))
    elif shutil.which("kgx"):  # GNOME Console
        helpers.append(WorkTool(
            id="terminal",
            title="Terminal",
            subtitle="Open a command window in this project",
            icon="utilities-terminal-symbolic",
            argv=("kgx", "--working-directory={path}"),
            metered=False,
            is_ai=False,
        ))

    recent = recent_ids()
    rank = {tid: i for i, tid in enumerate(recent)}
    catalog_order = {entry[0]: i for i, entry in enumerate(_CATALOG)}

    def sort_key(t: WorkTool) -> tuple:
        # AI with a recency rank first (lower index = more recent), then other
        # AI in catalog order (Claude before Aider), then helpers.
        cat = catalog_order.get(t.id, 999)
        if t.is_ai and t.id in rank:
            return (0, rank[t.id], cat)
        if t.is_ai:
            return (1, cat, 0)
        return (2, cat, 0)

    ordered = sorted(found, key=sort_key)
    return ordered + helpers


def _first_which(binaries: tuple[str, ...]) -> str | None:
    for name in binaries:
        hit = shutil.which(name)
        if hit:
            return hit
    return None


# --- recents ----------------------------------------------------------------

def recent_ids() -> list[str]:
    """Tool ids most-recently launched, newest first."""
    data = _load_recents()
    now = time.time()
    cutoff = now - RECENT_DAYS * 86400
    kept: list[tuple[str, float]] = []
    for row in data.get("recent", []):
        if not isinstance(row, dict):
            continue
        tid = row.get("id")
        at = row.get("at")
        if not isinstance(tid, str) or not isinstance(at, (int, float)):
            continue
        if at >= cutoff:
            kept.append((tid, float(at)))
    kept.sort(key=lambda p: p[1], reverse=True)
    # Unique, preserving order.
    out: list[str] = []
    seen: set[str] = set()
    for tid, _ in kept:
        if tid not in seen:
            seen.add(tid)
            out.append(tid)
    return out


def is_recent(tool_id: str) -> bool:
    return tool_id in recent_ids()


def record_use(tool_id: str) -> None:
    """Remember that this tool was chosen for a project."""
    if not tool_id:
        return
    data = _load_recents()
    rows = [
        r for r in data.get("recent", [])
        if isinstance(r, dict) and r.get("id") != tool_id
    ]
    rows.insert(0, {"id": tool_id, "at": time.time()})
    data["recent"] = rows[:RECENT_MAX]
    _save_recents(data)


def _load_recents() -> dict:
    try:
        return json.loads(RECENTS_FILE.read_text())
    except (OSError, ValueError):
        return {"recent": []}


def _save_recents(data: dict) -> None:
    try:
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        tmp = RECENTS_FILE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(data, indent=2) + "\n")
        tmp.replace(RECENTS_FILE)
    except OSError:
        # Preference file is best-effort; a full disk must not block launch.
        pass


# --- credits ----------------------------------------------------------------

def probe_credits(tool: WorkTool) -> CreditStatus:
    """Best-effort remaining-credit check for one tool."""
    probe = PROBES.get(tool.id)
    if probe is not None:
        return probe()
    # Files / Terminal are not credit products — empty label means the UI
    # keeps the action subtitle ("Open the project folder") as-is.
    if not tool.is_ai:
        return CreditStatus(ok=True, label="")
    if not tool.metered:
        return CreditStatus(ok=True, label="Ready to use")
    return CreditStatus(
        ok=None,
        label="Installed — open the app to check your plan",
    )


def probe_all(tools: list[WorkTool] | None = None) -> dict[str, CreditStatus]:
    """Probe every tool. Safe to call from a worker thread."""
    tools = tools if tools is not None else available_work_tools()
    return {t.id: probe_credits(t) for t in tools}


_LOW = re.compile(
    r"credit balance is too low|"
    r"out of extra usage|"
    r"usage limit reached|"
    r"you.?re out of|"
    r"no credits|"
    r"insufficient.?credit|"
    r"quota.?exceeded|"
    r"rate limit",
    re.I,
)
_OKISH = re.compile(
    r"remaining|available|balance|resets in|%\s*used|"
    r"\$\d|credits?\s*left|within (your )?limit",
    re.I,
)


def _run(argv: list[str], timeout: float = 8.0) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except FileNotFoundError:
        return 127, "not installed"
    except subprocess.TimeoutExpired:
        return 124, "timed out"
    except OSError as exc:
        return 1, str(exc)
    text = ((completed.stdout or "") + "\n" + (completed.stderr or "")).strip()
    return completed.returncode, text


def _probe_claude() -> CreditStatus:
    """Use `claude auth status` + `claude usage`."""
    if not shutil.which("claude"):
        return CreditStatus(ok=False, label="Claude is not installed")

    code, auth_text = _run(["claude", "auth", "status"], timeout=6)
    logged_in = False
    try:
        blob = auth_text
        start = blob.find("{")
        if start >= 0:
            data = json.loads(blob[start:])
            logged_in = bool(data.get("loggedIn"))
    except (json.JSONDecodeError, TypeError, ValueError):
        logged_in = '"loggedIn": true' in auth_text

    if not logged_in and code == 0 and "loggedIn" in auth_text:
        return CreditStatus(
            ok=False,
            label="Not signed in — open Claude to sign in",
        )

    _code, usage = _run(["claude", "usage"], timeout=10)
    text = usage.strip()
    if not text:
        if logged_in:
            return CreditStatus(ok=None, label="Signed in — usage unknown")
        return CreditStatus(ok=False, label="Not signed in — open Claude to sign in")

    first = text.splitlines()[0].strip()
    if "not logged in" in text.lower():
        return CreditStatus(
            ok=False,
            label="Not signed in — open Claude to sign in",
        )
    if _LOW.search(text):
        # Prefer a calm human line over the raw vendor message when we can.
        if "too low" in first.lower() or "out of" in first.lower():
            return CreditStatus(ok=False, label="No usage left right now")
        return CreditStatus(ok=False, label=first or "No usage left right now")

    if _OKISH.search(text) or _code == 0:
        label = first if len(first) <= 72 else first[:69] + "…"
        if not label or label.lower().startswith("usage:"):
            label = "Ready to use"
        elif "credit" in label.lower() and "too low" not in label.lower():
            label = "Ready to use"
        return CreditStatus(ok=True, label=label)

    return CreditStatus(ok=None, label="Couldn’t check Claude’s plan")


def _probe_cursor() -> CreditStatus:
    if not shutil.which("cursor") and not shutil.which("cursor-agent"):
        return CreditStatus(ok=False, label="Cursor isn’t installed")
    return CreditStatus(
        ok=None,
        label="Installed — open Cursor to check your plan",
    )


def _probe_generic_installed(name: str, binary: str) -> CreditStatus:
    if not shutil.which(binary):
        return CreditStatus(ok=False, label=f"{name} isn’t installed")
    return CreditStatus(
        ok=None,
        label=f"Installed — open {name} to check your plan",
    )


PROBES: dict[str, Callable[[], CreditStatus]] = {
    "claude": _probe_claude,
    "cursor": _probe_cursor,
    "windsurf": lambda: _probe_generic_installed("Windsurf", "windsurf"),
    "codex": lambda: _probe_generic_installed("Codex", "codex"),
    "gemini": lambda: _probe_generic_installed("Gemini", "gemini"),
    "aider": lambda: _probe_generic_installed("Aider", "aider"),
    "amp": lambda: _probe_generic_installed("Amp", "amp"),
    "crush": lambda: _probe_generic_installed("Crush", "crush"),
    "goose": lambda: _probe_generic_installed("Goose", "goose"),
}
