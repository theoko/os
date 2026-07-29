"""AI tools for 'work on …' goals, plus whether each one still has credits.

Search lists Claude / Cursor / VS Code when they are installed. Offering a tool
that is out of credits looks like it works until the person is halfway into a
session — so we probe each tool and put the answer on the row before they click.

Probes are best-effort and never raise. A probe that times out or is not
understood reports "unknown" rather than pretending the tool is free.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
from dataclasses import dataclass
from typing import Callable


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


def available_work_tools() -> list[WorkTool]:
    """AI / editor tools installed on this machine, Claude first.

    Only tools that actually exist are offered. An empty row for Cursor on a
    machine that never shipped Cursor is worse than a short list.
    """
    tools: list[WorkTool] = []
    if shutil.which("teddyos-claude") or shutil.which("claude"):
        # Prefer the windowed launcher so the person never sees a raw terminal.
        exe = shutil.which("teddyos-claude") or "claude"
        tools.append(WorkTool(
            id="claude",
            title="Claude",
            subtitle="Open this project in Claude Code",
            icon="teddyos-claude",
            argv=(exe, "{path}"),
            metered=True,
        ))
    if shutil.which("cursor"):
        tools.append(WorkTool(
            id="cursor",
            title="Cursor",
            subtitle="Open this project in Cursor",
            icon="text-editor",
            argv=("cursor", "{path}"),
            metered=True,
        ))
    if shutil.which("code"):
        tools.append(WorkTool(
            id="code",
            title="VS Code",
            subtitle="Open this project in VS Code",
            icon="text-editor",
            argv=("code", "{path}"),
            metered=False,
        ))
    # Always offer Files so "work on X" can still open the folder when no AI
    # tool is installed.
    file_mgr = (
        shutil.which("nautilus")
        or shutil.which("xdg-open")
        or "xdg-open"
    )
    tools.append(WorkTool(
        id="files",
        title="Files",
        subtitle="Open the project folder",
        icon="folder",
        argv=(file_mgr, "{path}"),
        metered=False,
    ))
    if shutil.which("gnome-terminal"):
        tools.append(WorkTool(
            id="terminal",
            title="Terminal",
            subtitle="Open a terminal in this project",
            icon="utilities-terminal-symbolic",
            argv=("gnome-terminal", "--working-directory={path}"),
            metered=False,
        ))
    return tools


def probe_credits(tool: WorkTool) -> CreditStatus:
    """Best-effort remaining-credit check for one tool."""
    if tool.id == "claude":
        return _probe_claude()
    if tool.id == "cursor":
        return _probe_cursor()
    if not tool.metered:
        return CreditStatus(ok=True, label="No credits needed")
    return CreditStatus(ok=None, label="Credits unknown")


def probe_all(tools: list[WorkTool] | None = None) -> dict[str, CreditStatus]:
    """Probe every tool. Safe to call from a worker thread."""
    tools = tools if tools is not None else available_work_tools()
    return {t.id: probe_credits(t) for t in tools}


# --- probes -----------------------------------------------------------------

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
            env=None,  # inherit PATH so claude from /usr/local/bin is found
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
    """Use `claude auth status` + `claude usage`.

    `claude usage` is the account-level check when signed in. Observed outputs:
      - "Credit balance is too low"
      - "Not logged in · Please run /login"
      - session cost tables (still means the account answered)
    """
    if not shutil.which("claude"):
        return CreditStatus(ok=False, label="Claude is not installed")

    code, auth_text = _run(["claude", "auth", "status"], timeout=6)
    logged_in = False
    try:
        # Prefer the last JSON object in the stream — banners sometimes precede it.
        blob = auth_text
        start = blob.find("{")
        if start >= 0:
            data = json.loads(blob[start:])
            logged_in = bool(data.get("loggedIn"))
    except (json.JSONDecodeError, TypeError, ValueError):
        logged_in = "loggedIn\": true" in auth_text or '"loggedIn": true' in auth_text

    if not logged_in:
        # API-key auth still reports loggedIn true on some builds; if status
        # failed entirely, keep going to `usage` which is more decisive.
        if code == 0 and "loggedIn" in auth_text:
            return CreditStatus(
                ok=False,
                label="Not signed in — open Claude to log in",
            )

    code, usage = _run(["claude", "usage"], timeout=10)
    text = usage.strip()
    if not text:
        if logged_in:
            return CreditStatus(ok=None, label="Signed in · credits unknown")
        return CreditStatus(ok=False, label="Not signed in — open Claude to log in")

    first = text.splitlines()[0].strip()
    low = _LOW.search(text)
    if low or "not logged in" in text.lower():
        if "not logged in" in text.lower():
            return CreditStatus(
                ok=False,
                label="Not signed in — open Claude to log in",
            )
        return CreditStatus(ok=False, label=first or "No credits left")

    if _OKISH.search(text) or code == 0:
        # Keep the message short enough for a list row.
        label = first if len(first) <= 72 else first[:69] + "…"
        if not label or label.lower().startswith("usage:"):
            label = "Credits available"
        else:
            label = f"Credits · {label}" if "credit" not in label.lower() else label
        return CreditStatus(ok=True, label=label)

    return CreditStatus(ok=None, label="Could not check Claude credits")


def _probe_cursor() -> CreditStatus:
    """Cursor does not expose a stable CLI for remaining fast requests.

    We can only say it is installed. Pretending we know the balance would be
    a lie the person notices the first time a request is refused.
    """
    if not shutil.which("cursor"):
        return CreditStatus(ok=False, label="Cursor is not installed")
    return CreditStatus(
        ok=None,
        label="Installed · open Cursor to see remaining credits",
    )


# Probe registry for tests / extension.
PROBES: dict[str, Callable[[], CreditStatus]] = {
    "claude": _probe_claude,
    "cursor": _probe_cursor,
}
