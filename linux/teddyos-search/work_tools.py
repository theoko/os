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
# Tools the person just finished signing into (this session). Soft probes
# (Gemini/Antigravity) never flip ok=True on their own — without this mark,
# Get help stays stuck on Continue after a successful browser login.
SESSION_READY_FILE = CONFIG_DIR / "session-ready.json"
_SESSION_READY_TTL_SEC = 12 * 3600


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

    label:
      Short plain-language line for the UI. Prefer real balance/plan text
      when we have it (e.g. “Max plan · ready”, “Out of uses for now”).
    """
    ok: bool | None
    label: str


# Catalog of tools we know how to launch. Order is the default preference when
# nothing has been used yet (Claude first). Only entries whose binary exists
# (PATH, ~/.local/bin, desktop file, flatpak) are offered.
#
# Each entry: id, title, subtitle, icon, metered, is_ai, candidate binaries,
# argv after the binary ({path} = project dir), optional .desktop basenames.
_CATALOG: list[tuple] = [
    (
        # binaries = commands that mean "this tool is installed"
        # Terminal TUIs are launched via teddyos-agent (see _argv_for).
        "claude", "Claude", "Ready · tap to open",
        "teddyos-claude", True, True,
        ("claude", "teddyos-claude"),
        ("{path}",),
        ("teddyos-claude.desktop",),
    ),
    (
        # Official xAI Grok Build CLI (`npm i -g @xai-official/grok` → `grok`).
        "grok", "Grok", "Ready · tap to open",
        "teddyos-grok", True, True,
        ("grok",),
        ("{path}",),
        (),
    ),
    (
        # GitHub Copilot CLI (`npm i -g @github/copilot` → `copilot`).
        "copilot", "Copilot", "Ready · tap to open",
        "teddyos-copilot", True, True,
        ("copilot",),
        ("{path}",),
        (),
    ),
    (
        # Google Antigravity CLI (`agy` — curl install from antigravity.google).
        "antigravity", "Antigravity", "Ready · tap to open",
        "teddyos-antigravity", True, True,
        ("agy", "antigravity"),
        ("{path}",),
        (),
    ),
    (
        "cursor", "Cursor", "Open in the Cursor app",
        "teddyos-cursor", True, True,
        ("cursor", "cursor-agent"),
        ("{path}",),
        (
            "cursor.desktop",
            "Cursor.desktop",
            "cursor-url-handler.desktop",
            "code-cursor.desktop",
        ),
    ),
    (
        # Perplexity: web app wrapper (always on image) or local desktop binary.
        # Perplexity: Chromium --app wrapper — no separate download.
        "perplexity", "Perplexity", "Web app · no download needed",
        "teddyos-perplexity", False, True,
        ("teddyos-perplexity", "perplexity", "pplx"),
        (),
        (
            "teddyos-perplexity.desktop",
            "perplexity.desktop",
            "Perplexity.desktop",
        ),
    ),
    (
        # Devin (Cognition): web app wrapper — no separate download / AppImage.
        "devin", "Devin", "Web app · no download needed",
        "teddyos-devin", False, True,
        ("teddyos-devin", "devin"),
        (),
        (
            "teddyos-devin.desktop",
            "devin.desktop",
            "Devin.desktop",
        ),
    ),
    (
        # Replit: web app wrapper (native binary used only if user installed one).
        "replit", "Replit", "Web app · no download needed",
        "teddyos-replit", False, True,
        ("teddyos-replit", "replit", "Replit"),
        (),
        (
            "teddyos-replit.desktop",
            "replit.desktop",
            "Replit.desktop",
        ),
    ),
    (
        "windsurf", "Windsurf", "Open in the Windsurf app",
        "text-editor", True, True,
        ("windsurf", "windsurf-bin"),
        ("{path}",),
        ("windsurf.desktop", "Windsurf.desktop"),
    ),
    (
        "codex", "Codex", "Ready · tap to open",
        "teddyos-codex", True, True,
        ("codex",),
        ("{path}",),
        (),
    ),
    (
        "gemini", "Gemini", "Ready · tap to open",
        "teddyos-gemini", True, True,
        ("gemini",),
        ("{path}",),
        (),
    ),
    (
        "aider", "Aider", "Ready · tap to open",
        "utilities-terminal-symbolic", True, True,
        ("aider",),
        ("{path}",),
        (),
    ),
    (
        "amp", "Amp", "Open this project with Amp",
        "text-editor", True, True,
        ("amp",),
        ("{path}",),
        (),
    ),
    (
        "crush", "Crush", "Open this project with Crush",
        "utilities-terminal-symbolic", True, True,
        ("crush",),
        ("{path}",),
        (),
    ),
    (
        "goose", "Goose", "Open this project with Goose",
        "utilities-terminal-symbolic", True, True,
        ("goose",),
        ("{path}",),
        (),
    ),
    (
        "ollama", "Ollama", "Chat with a local model in this project",
        "utilities-terminal-symbolic", False, True,
        ("ollama",),
        ("{path}",),
        (),
    ),
    (
        "code", "VS Code", "Open this project in VS Code",
        "text-editor", False, True,
        ("code", "code-insiders", "code-oss"),
        ("{path}",),
        (
            "code.desktop",
            "code-url-handler.desktop",
            "code-oss.desktop",
            "com.visualstudio.code.desktop",
            "visual-studio-code.desktop",
        ),
    ),
    (
        "codium", "VSCodium", "Open this project in VSCodium",
        "text-editor", False, True,
        ("codium", "vscodium"),
        ("{path}",),
        ("codium.desktop", "vscodium.desktop"),
    ),
    (
        "zed", "Zed", "Open this project in Zed",
        "text-editor", False, True,
        ("zed", "zeditor"),
        ("{path}",),
        ("zed.desktop", "dev.zed.Zed.desktop"),
    ),
]

# .desktop basenames we map to catalog ids (also scanned the other way).
_DESKTOP_TO_ID: dict[str, str] = {}
for _e in _CATALOG:
    for _desk in _e[8]:
        _DESKTOP_TO_ID[_desk.lower()] = _e[0]

# Interactive CLI tools that need a TTY window. Without teddyos-agent they
# print "stdin is not a terminal" and the Search click does nothing.
_TTY_TOOLS = frozenset({
    "claude", "grok", "gemini", "codex", "copilot", "antigravity",
    "aider", "ollama", "crush", "goose",
})

# Tools that accept an initial prompt in an interactive session. GUI editors
# (Cursor, VS Code) and web apps (Perplexity) are not included — they open
# folders or a browser, not a project chat with argv.
_PROMPT_TOOLS = frozenset({
    "claude", "grok", "gemini", "codex", "copilot", "antigravity", "aider",
})


def prompt_argv(tool_id: str, prompt: str) -> list[str]:
    """CLI args that inject `prompt` into an interactive session.

    Prefer flags that keep the session open (chat continues) over headless
    one-shot modes. Empty prompt → no args.
    """
    text = (prompt or "").strip()
    if not text or tool_id not in _PROMPT_TOOLS:
        return []
    # Gemini / Antigravity: -i runs the prompt then stays interactive.
    if tool_id in ("gemini", "antigravity"):
        return ["-i", text]
    # Claude, Grok, Codex, Copilot, Aider: positional initial prompt.
    return [text]


def mark_session_ready(tool_id: str) -> None:
    """Remember a successful sign-in so Get help unlocks without a perfect probe."""
    tid = (tool_id or "").strip()
    if not tid:
        return
    try:
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        data: dict = {}
        if SESSION_READY_FILE.is_file():
            try:
                data = json.loads(SESSION_READY_FILE.read_text())
            except (OSError, json.JSONDecodeError, TypeError):
                data = {}
        ready = data.get("ready") if isinstance(data.get("ready"), dict) else {}
        ready[tid] = time.time()
        # Drop stale entries
        now = time.time()
        ready = {
            k: float(v)
            for k, v in ready.items()
            if now - float(v) < _SESSION_READY_TTL_SEC
        }
        SESSION_READY_FILE.write_text(json.dumps({"ready": ready}, indent=2) + "\n")
    except OSError:
        pass


def is_session_ready(tool_id: str) -> bool:
    tid = (tool_id or "").strip()
    if not tid:
        return False
    try:
        data = json.loads(SESSION_READY_FILE.read_text())
        ready = data.get("ready") if isinstance(data, dict) else None
        if not isinstance(ready, dict) or tid not in ready:
            return False
        return (time.time() - float(ready[tid])) < _SESSION_READY_TTL_SEC
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        return False


def ready_for_broadcast(
    tool: WorkTool,
    status: CreditStatus | None,
) -> bool:
    """True when this tool should receive a shared "ask all" prompt.

    Only helpers we know are signed in / ready. Including "unknown" used to
    fire Grok/Codex while logged out and show useless permission errors.
    Session-ready (just finished Connect) unlocks soft-probe tools.
    """
    if tool.id not in _PROMPT_TOOLS or not tool.is_ai:
        return False
    if status is not None and status.ok is True:
        return True
    # Recent successful Connect unlocks soft/unknown/stale "needs sign-in"
    # probes so Get help is seamless right after the browser dance.
    if is_session_ready(tool.id):
        if status is not None and status.ok is False:
            lab = (status.label or "").lower()
            if "not on this computer" in lab or "isn't installed" in lab:
                return False
        return True
    if status is None:
        return False
    return False


def tools_ready_for_broadcast(
    tools: list[WorkTool],
    statuses: dict[str, CreditStatus] | None = None,
) -> list[WorkTool]:
    """Filter to chat AIs that can take a simultaneous prompt."""
    statuses = statuses or {}
    return [t for t in tools if ready_for_broadcast(t, statuses.get(t.id))]


def is_chat_helper(tool_id: str) -> bool:
    """True for helpers that answer a typed question (not editors/folders)."""
    return tool_id in _PROMPT_TOOLS


def available_work_tools() -> list[WorkTool]:
    """Installed tools, recently used AI first, then the rest of the catalog.

    Files and Terminal always trail — they are escapes, not the point of a
    "work on …" goal.
    """
    found: list[WorkTool] = []
    seen_ids: set[str] = set()
    desktop_execs = _desktop_exec_map()

    for entry in _CATALOG:
        tid, title, subtitle, icon, metered, is_ai, binaries, args, desks = entry
        exe = _resolve_binary(binaries, desks, desktop_execs)
        if not exe:
            continue
        argv = _argv_for(tid, exe, args)
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
            subtitle="See the project’s files",
            icon="folder-symbolic",
            argv=(file_mgr, "{path}"),
            metered=False,
            is_ai=False,
        ),
    ]
    if shutil.which("gnome-terminal"):
        helpers.append(WorkTool(
            id="terminal",
            title="Command window",
            subtitle="For advanced use",
            icon="utilities-terminal-symbolic",
            argv=("gnome-terminal", "--working-directory={path}"),
            metered=False,
            is_ai=False,
        ))
    elif shutil.which("kgx"):  # GNOME Console
        helpers.append(WorkTool(
            id="terminal",
            title="Command window",
            subtitle="For advanced use",
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


def _argv_for(tool_id: str, exe: str, args: tuple[str, ...]) -> tuple[str, ...]:
    """Build argv. Terminal AI tools must open through teddyos-agent (VTE).

    Passing a project path as a CLI argument is wrong for Gemini/Codex — they
    treat it as a prompt. The wrapper takes the folder as cwd instead.
    """
    agent = shutil.which("teddyos-agent")
    if tool_id in _TTY_TOOLS and agent:
        # teddyos-claude remains preferred for Claude (same UX, stable app id).
        if tool_id == "claude":
            dedicated = shutil.which("teddyos-claude")
            if dedicated:
                return (dedicated, "{path}")
        return (agent, tool_id, "{path}")
    return (exe,) + tuple(args)


def _extra_bin_dirs() -> list[Path]:
    """Places tools often land that are not always on PATH for GUI apps."""
    home = Path.home()
    dirs = [
        home / ".local" / "bin",
        Path("/usr/local/bin"),
        Path("/opt/homebrew/bin"),  # harmless on Linux
        Path("/snap/bin"),
    ]
    # Node global bins (Claude / Codex / Gemini on the image).
    for base in (Path("/usr/local/lib/nodejs"), home / ".npm-global" / "bin"):
        if base.is_dir():
            for child in base.iterdir():
                if child.is_dir():
                    b = child / "bin"
                    if b.is_dir():
                        dirs.append(b)
    return dirs


def _resolve_binary(
    binaries: tuple[str, ...],
    desktops: tuple[str, ...],
    desktop_execs: dict[str, str],
) -> str | None:
    for name in binaries:
        hit = shutil.which(name)
        if hit:
            return hit
        for d in _extra_bin_dirs():
            cand = d / name
            if cand.is_file() and os.access(cand, os.X_OK):
                return str(cand)
    for desk in desktops:
        exe = desktop_execs.get(desk.lower())
        if exe:
            return exe
    return None


def _desktop_exec_map() -> dict[str, str]:
    """basename.desktop (lower) → first executable token from Exec=."""
    out: dict[str, str] = {}
    search_dirs = [
        Path("/usr/share/applications"),
        Path("/usr/local/share/applications"),
        Path.home() / ".local" / "share" / "applications",
        Path("/var/lib/flatpak/exports/share/applications"),
        Path.home() / ".local" / "share" / "flatpak" / "exports" / "share" / "applications",
    ]
    for d in search_dirs:
        if not d.is_dir():
            continue
        try:
            entries = list(d.glob("*.desktop"))
        except OSError:
            continue
        for path in entries:
            key = path.name.lower()
            if key not in _DESKTOP_TO_ID and not any(
                k in key for k in (
                    "cursor", "code", "codium", "windsurf", "zed", "claude",
                    "perplexity", "copilot", "antigravity", "agy",
                )
            ):
                continue
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            exe = _parse_desktop_exec(text)
            if exe:
                out[key] = exe
                # Also map by catalog id if we know this desktop file.
                tid = _DESKTOP_TO_ID.get(key)
                if tid:
                    out[key] = exe
    return out


def _parse_desktop_exec(text: str) -> str | None:
    for line in text.splitlines():
        if not line.startswith("Exec="):
            continue
        raw = line[5:].strip()
        # Drop field codes %f %F %u %U %i %c %k
        parts = [p for p in raw.split() if not p.startswith("%")]
        if not parts:
            continue
        cmd = parts[0]
        # flatpak run app.id → keep full argv as single string? We only need
        # one executable for argv[0]; flatpak needs multi-token.
        if cmd == "flatpak" and len(parts) >= 3 and parts[1] == "run":
            # Store as "flatpak\0run\0app" joined later — use a marker path.
            # Simpler: return the app binary if /var/lib/flatpak has it.
            return " ".join(parts)  # handled specially in launch if space
        if cmd.startswith("/"):
            return cmd if Path(cmd).exists() or True else cmd
        hit = shutil.which(cmd)
        return hit or cmd
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
        st = probe()
    elif not tool.is_ai:
        # Files / Terminal — empty label keeps the action subtitle as-is.
        return CreditStatus(ok=True, label="")
    elif not tool.metered:
        st = CreditStatus(ok=True, label="Ready · tap to open")
    else:
        st = CreditStatus(ok=None, label="Ready · tap to open")
    # After Connect, soft probes often stay unknown — treat as ready so
    # Get help / Draft my replies is not stuck on Continue.
    if st.ok is not True and is_session_ready(tool.id):
        if st.ok is False and "not on this computer" in (st.label or "").lower():
            return st
        return CreditStatus(ok=True, label="Connected · ready")
    return st


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


def _parse_balance_snippet(text: str) -> str | None:
    """Pull a short human balance line from CLI usage output."""
    if not text:
        return None
    # Prefer first non-empty line that looks like balance / plan / %.
    for line in text.splitlines():
        line = line.strip()
        if not line or len(line) > 90:
            continue
        low = line.lower()
        if any(
            s in low
            for s in (
                "not sure which", "quick options", "built-in",
                "ambiguous", "say which", "usage of some",
            )
        ):
            return None  # interactive help, not real usage
        if _LOW.search(line):
            return "Out of uses for now"
        if re.search(
            r"\d+\s*%|\d+\s*/\s*\d+|remaining|left|balance|resets|"
            r"\$\d|credits?",
            low,
        ):
            # Clean decorative bullets
            return re.sub(r"^[\s•\-\*]+", "", line)[:72]
    return None


def _claude_plan_label(data: dict) -> str | None:
    sub = (data.get("subscriptionType") or data.get("subscription") or "")
    sub = str(sub).strip().lower()
    if not sub:
        return None
    pretty = {
        "max": "Max plan",
        "pro": "Pro plan",
        "team": "Team plan",
        "enterprise": "Enterprise",
        "free": "Free plan",
    }.get(sub, sub.title() + " plan")
    return pretty


def _probe_claude() -> CreditStatus:
    """Use `claude auth status` + `claude usage` when it returns real numbers."""
    if not shutil.which("claude"):
        return CreditStatus(ok=False, label="Not on this computer")

    code, auth_text = _run(["claude", "auth", "status"], timeout=6)
    logged_in = False
    plan: str | None = None
    try:
        blob = auth_text
        start = blob.find("{")
        if start >= 0:
            data = json.loads(blob[start:])
            logged_in = bool(data.get("loggedIn"))
            plan = _claude_plan_label(data)
    except (json.JSONDecodeError, TypeError, ValueError):
        logged_in = '"loggedIn": true' in auth_text
        if '"subscriptionType": "max"' in auth_text.lower():
            plan = "Max plan"
        elif '"subscriptionType": "pro"' in auth_text.lower():
            plan = "Pro plan"

    if not logged_in:
        # JSON may say loggedIn false, or no status at all.
        if code != 0 or "loggedIn" in auth_text or "not logged" in auth_text.lower():
            return CreditStatus(
                ok=False,
                label="Needs sign-in · tap to connect",
            )

    _code, usage = _run(["claude", "usage"], timeout=10)
    text = (usage or "").strip()
    if "not logged in" in text.lower():
        return CreditStatus(
            ok=False,
            label="Needs sign-in · tap to connect",
        )
    if _LOW.search(text):
        # Real low-balance line (API-key style credits).
        return CreditStatus(ok=False, label="Out of uses for now")

    snippet = _parse_balance_snippet(text)
    if snippet and snippet != "Out of uses for now":
        # e.g. "72% remaining" or "Resets in 4h"
        if plan:
            return CreditStatus(ok=True, label=f"{plan} · {snippet}")
        return CreditStatus(ok=True, label=snippet)

    # Subscription (Max/Pro): `claude usage` often refuses headless quota.
    if logged_in and plan:
        return CreditStatus(ok=True, label=f"{plan} · ready")
    if logged_in:
        return CreditStatus(ok=True, label="Connected · ready")
    return CreditStatus(ok=False, label="Needs sign-in · tap to connect")


def _probe_cursor() -> CreditStatus:
    if not shutil.which("cursor") and not shutil.which("cursor-agent"):
        return CreditStatus(ok=False, label="Not on this computer")
    return CreditStatus(
        ok=None,
        label="Ready · tap to open",
    )


def _probe_grok() -> CreditStatus:
    """Grok needs an xAI login — installed binary is not enough."""
    if not shutil.which("grok"):
        found = False
        for d in _extra_bin_dirs():
            if (d / "grok").is_file():
                found = True
                break
        if not found:
            return CreditStatus(ok=False, label="Not on this computer")
    # Prefer a status/login command if present (faster than -p ping).
    for argv in (
        ["grok", "auth", "status"],
        ["grok", "status"],
        ["grok", "whoami"],
    ):
        code, text = _run(argv, timeout=6)
        low = (text or "").lower()
        if not text:
            continue
        if any(s in low for s in ("not signed", "not logged", "unauthenticated")):
            return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
        if _LOW.search(text):
            return CreditStatus(ok=False, label="Out of uses for now")
        snippet = _parse_balance_snippet(text)
        if snippet:
            return CreditStatus(ok=True, label=snippet)
        if code == 0 and any(s in low for s in ("signed", "logged", "ok", "user")):
            return CreditStatus(ok=True, label="Connected · ready")

    code, text = _run(["grok", "-p", "ping"], timeout=12)
    low = (text or "").lower()
    if "not signed in" in low:
        return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
    if _LOW.search(text or ""):
        return CreditStatus(ok=False, label="Out of uses for now")
    if code == 0:
        return CreditStatus(ok=True, label="Connected · ready")
    if any(s in low for s in ("auth", "login", "sign in", "authenticate")):
        return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
    return CreditStatus(ok=False, label="Needs sign-in · tap to connect")


def _probe_codex() -> CreditStatus:
    if not shutil.which("codex"):
        return CreditStatus(ok=False, label="Not on this computer")
    code, text = _run(["codex", "login", "status"], timeout=8)
    low = (text or "").lower()
    if "not logged in" in low or (code != 0 and "logged in" not in low):
        return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
    if code == 0 and "logged in" in low and "not logged" not in low:
        # "Logged in using ChatGPT" / API key
        if "chatgpt" in low:
            return CreditStatus(ok=True, label="ChatGPT · connected")
        if "api" in low:
            return CreditStatus(ok=True, label="API key · connected")
        return CreditStatus(ok=True, label="Connected · ready")
    return CreditStatus(ok=False, label="Needs sign-in · tap to connect")


def _probe_copilot() -> CreditStatus:
    if not shutil.which("copilot"):
        return CreditStatus(ok=False, label="Not on this computer")
    # Prefer explicit auth if the CLI exposes it (fast paths only).
    for argv in (
        ["copilot", "auth", "status"],
        ["copilot", "status"],
    ):
        code, text = _run(argv, timeout=6)
        low = (text or "").lower()
        if not text or code == 127:
            continue
        if any(s in low for s in ("not logged", "not signed", "unauthenticated")):
            return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
        if _LOW.search(text):
            return CreditStatus(ok=False, label="Out of uses for now")
        snippet = _parse_balance_snippet(text)
        if snippet:
            return CreditStatus(ok=True, label=snippet)
        if code == 0 and any(
            s in low for s in ("logged in", "signed in", "authenticated", "ok")
        ):
            return CreditStatus(ok=True, label="Connected · ready")

    # Lightweight fallback: config with credentials.
    home = Path.home() / ".copilot"
    if not home.is_dir():
        return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
    markers = list(home.glob("**/*"))
    if len(markers) < 2:
        return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
    return CreditStatus(ok=True, label="Connected · ready")


def _probe_generic_installed(name: str, binary: str) -> CreditStatus:
    if not shutil.which(binary):
        # Still look in the extra dirs GUI apps often miss.
        found = False
        for d in _extra_bin_dirs():
            cand = d / binary
            if cand.is_file() and os.access(cand, os.X_OK):
                found = True
                break
        if not found:
            return CreditStatus(ok=False, label="Not on this computer")
    # Installed but we cannot prove login — do not mark ready for Get help.
    return CreditStatus(
        ok=None,
        label="May need a one-time sign-in",
    )


def _probe_any_binary(name: str, binaries: tuple[str, ...]) -> CreditStatus:
    for b in binaries:
        if shutil.which(b):
            return CreditStatus(
                ok=None,
                label="May need a one-time sign-in",
            )
        for d in _extra_bin_dirs():
            cand = d / b
            if cand.is_file() and os.access(cand, os.X_OK):
                return CreditStatus(
                    ok=None,
                    label="May need a one-time sign-in",
                )
    return CreditStatus(ok=False, label="Not on this computer")


def _have_chromium() -> bool:
    return any(
        shutil.which(n)
        for n in (
            "chromium", "chromium-browser",
            "google-chrome", "google-chrome-stable",
        )
    )


def _probe_web_app(wrapper: str, *, profile: str = "") -> CreditStatus:
    """Devin / Replit / Perplexity — no download; Chromium --app wrappers.

    They are always “installed” when the wrapper + browser exist. Cookie
    sign-in is optional and happens inside the window.
    """
    if not shutil.which(wrapper):
        # Wrapper missing from image — rare; not “needs download” for the user.
        return CreditStatus(
            ok=False,
            label="Not on this computer · reinstall teddyOS",
        )
    if not _have_chromium():
        return CreditStatus(
            ok=False,
            label="Needs a browser · install Chromium",
        )
    # Optional: nicer label if they already signed in once.
    if profile:
        cookies = (
            Path.home() / ".config" / profile / "Default" / "Cookies"
        )
        if cookies.is_file() and cookies.stat().st_size > 0:
            return CreditStatus(ok=True, label="Web app · signed in before")
    return CreditStatus(ok=True, label="Web app · tap to open")


def _probe_gemini() -> CreditStatus:
    if not shutil.which("gemini"):
        return CreditStatus(ok=False, label="Not on this computer")
    for argv in (
        ["gemini", "auth", "status"],
        ["gemini", "whoami"],
    ):
        code, text = _run(argv, timeout=6)
        low = (text or "").lower()
        if not text:
            continue
        if any(s in low for s in ("not logged", "not signed", "unauthenticated")):
            return CreditStatus(ok=False, label="Needs sign-in · tap to connect")
        if _LOW.search(text):
            return CreditStatus(ok=False, label="Out of uses for now")
        snippet = _parse_balance_snippet(text)
        if snippet:
            return CreditStatus(ok=True, label=snippet)
        if code == 0:
            return CreditStatus(ok=True, label="Connected · ready")
    # Credential crumbs Google tools often leave.
    for p in (
        Path.home() / ".gemini",
        Path.home() / ".config" / "gemini",
    ):
        if p.is_dir() and any(p.iterdir()):
            return CreditStatus(ok=True, label="Connected · ready")
    return CreditStatus(
        ok=None,
        label="May need sign-in · use Connect if it fails",
    )


PROBES: dict[str, Callable[[], CreditStatus]] = {
    "claude": _probe_claude,
    "grok": _probe_grok,
    "copilot": _probe_copilot,
    "antigravity": lambda: (
        _probe_any_binary("Antigravity", ("agy", "antigravity"))
        if not is_session_ready("antigravity")
        else CreditStatus(ok=True, label="Connected · ready")
    ),
    "cursor": _probe_cursor,
    "perplexity": lambda: _probe_web_app(
        "teddyos-perplexity", profile="teddyos-perplexity",
    ),
    "devin": lambda: _probe_web_app(
        "teddyos-devin", profile="teddyos-devin",
    ),
    "replit": lambda: _probe_web_app(
        "teddyos-replit", profile="teddyos-replit",
    ),
    "windsurf": lambda: _probe_generic_installed("Windsurf", "windsurf"),
    "codex": _probe_codex,
    "gemini": _probe_gemini,
    "aider": lambda: _probe_generic_installed("Aider", "aider"),
    "amp": lambda: _probe_generic_installed("Amp", "amp"),
    "crush": lambda: _probe_generic_installed("Crush", "crush"),
    "goose": lambda: _probe_generic_installed("Goose", "goose"),
}
