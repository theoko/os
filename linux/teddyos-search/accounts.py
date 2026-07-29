"""Sign-in catalog for teddyOS helpers — plain language, no terminal required.

Each helper has its own vendor login (browser OAuth almost always). This module
knows *which* command starts that flow and whether the person looks connected,
so a single “Connect” button can do the right thing without anyone learning CLI.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


@dataclass(frozen=True)
class Account:
    id: str
    title: str
    # One line under the name — who it is, in normal words.
    blurb: str
    icon: str
    # Binaries that mean “this is installed”.
    binaries: tuple[str, ...]
    # Argv that starts browser / device sign-in (run in a guided window).
    connect_argv: tuple[str, ...]
    # Short instruction shown above the sign-in window.
    connect_hint: str
    # Optional desktop-only helpers (e.g. Perplexity web).
    always_available: bool = False


# Order is the order people see in Connect your helpers.
_ACCOUNTS: list[Account] = [
    Account(
        id="claude",
        title="Claude",
        blurb="From Anthropic — helps write and change code",
        icon="teddyos-claude",
        binaries=("claude", "teddyos-claude"),
        connect_argv=("claude", "auth", "login"),
        connect_hint=(
            "A browser window will open so you can sign in to Claude. "
            "When you’re done there, come back here — you can close this window."
        ),
    ),
    Account(
        id="grok",
        title="Grok",
        blurb="From xAI — helps write and change code",
        icon="teddyos-grok",
        binaries=("grok",),
        connect_argv=("grok", "login", "--oauth"),
        connect_hint=(
            "A browser window will open for Grok (xAI). "
            "Finish sign-in there, then come back here."
        ),
    ),
    Account(
        id="gemini",
        title="Gemini",
        blurb="From Google — helps write and change code",
        icon="teddyos-gemini",
        binaries=("gemini",),
        # First launch walks through Google sign-in interactively.
        connect_argv=("gemini",),
        connect_hint=(
            "Gemini will ask you to sign in with Google. "
            "Follow the steps on screen; a browser may open."
        ),
    ),
    Account(
        id="codex",
        title="Codex",
        blurb="From OpenAI — helps write and change code",
        icon="teddyos-codex",
        binaries=("codex",),
        connect_argv=("codex", "login"),
        connect_hint=(
            "A browser window will open so you can sign in to OpenAI / Codex. "
            "Finish there, then come back here."
        ),
    ),
    Account(
        id="copilot",
        title="Copilot",
        blurb="From GitHub — helps write and change code",
        icon="teddyos-copilot",
        binaries=("copilot",),
        connect_argv=("copilot", "login"),
        connect_hint=(
            "You’ll get a code and a browser link for GitHub Copilot. "
            "Open the link, enter the code, then come back here."
        ),
    ),
    Account(
        id="antigravity",
        title="Antigravity",
        blurb="From Google — agent that works in your project",
        icon="teddyos-antigravity",
        binaries=("agy", "antigravity"),
        connect_argv=("agy",),
        connect_hint=(
            "Antigravity will open a browser for Google sign-in. "
            "Finish there, then come back here."
        ),
    ),
    Account(
        id="perplexity",
        title="Perplexity",
        blurb="Answers questions on the web",
        icon="teddyos-perplexity",
        binaries=("teddyos-perplexity", "perplexity"),
        connect_argv=("teddyos-perplexity",),
        connect_hint=(
            "Perplexity opens in a simple browser window. "
            "Sign in there if you have an account — or just start asking."
        ),
        always_available=True,
    ),
    Account(
        id="github",
        title="GitHub",
        blurb="Download your projects onto this computer",
        icon="user-info-symbolic",
        binaries=("gh",),
        # Web flow — no device codes to copy by hand when possible.
        connect_argv=("gh", "auth", "login", "-p", "https", "-w"),
        connect_hint=(
            "A browser window will open for GitHub. "
            "Sign in and approve teddyOS, then come back here."
        ),
    ),
]


def all_accounts() -> list[Account]:
    return list(_ACCOUNTS)


def get_account(account_id: str) -> Account | None:
    for a in _ACCOUNTS:
        if a.id == account_id:
            return a
    return None


def _which_any(names: tuple[str, ...]) -> str | None:
    home = Path.home()
    extras = [
        home / ".local" / "bin",
        Path("/usr/local/bin"),
    ]
    for name in names:
        hit = shutil.which(name)
        if hit:
            return hit
        for d in extras:
            cand = d / name
            if cand.is_file() and os.access(cand, os.X_OK):
                return str(cand)
    return None


def is_installed(account: Account) -> bool:
    if account.always_available and account.id == "perplexity":
        return bool(
            _which_any(account.binaries)
            or shutil.which("chromium")
            or shutil.which("chromium-browser")
        )
    return _which_any(account.binaries) is not None


def resolve_connect_argv(account: Account) -> list[str] | None:
    """Full argv with absolute binary path, or None if not installable."""
    if not account.connect_argv:
        return None
    head = account.connect_argv[0]
    # Prefer the real binary for the first token.
    if head in account.binaries or head in {b for b in account.binaries}:
        exe = _which_any(account.binaries) or _which_any((head,))
    else:
        exe = _which_any((head,)) or shutil.which(head)
    if not exe:
        # Perplexity fallback: open chromium app URL without wrapper.
        if account.id == "perplexity":
            browser = (
                shutil.which("chromium")
                or shutil.which("chromium-browser")
            )
            if browser:
                return [
                    browser,
                    "--app=https://www.perplexity.ai/",
                    "--class=teddyos-perplexity",
                ]
        return None
    return [exe, *account.connect_argv[1:]]


@dataclass(frozen=True)
class AccountStatus:
    """ok True = signed in, False = needs connect, None = installed unknown."""
    ok: bool | None
    label: str


def status_for(account: Account) -> AccountStatus:
    if not is_installed(account):
        return AccountStatus(ok=False, label="Not on this computer")
    probe = _STATUS_PROBES.get(account.id)
    if probe is None:
        return AccountStatus(ok=None, label="Ready · connect if asked")
    return probe()


def _run(argv: list[str], timeout: float = 8.0) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as exc:
        return 1, str(exc)
    text = ((completed.stdout or "") + "\n" + (completed.stderr or "")).strip()
    return completed.returncode, text


def _status_claude() -> AccountStatus:
    exe = _which_any(("claude",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    code, text = _run([exe, "auth", "status"], timeout=6)
    logged_in = False
    try:
        start = text.find("{")
        if start >= 0:
            data = json.loads(text[start:])
            logged_in = bool(data.get("loggedIn"))
    except (json.JSONDecodeError, TypeError, ValueError):
        logged_in = '"loggedIn": true' in text or "Logged in" in text
    if logged_in:
        return AccountStatus(ok=True, label="Connected")
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_grok() -> AccountStatus:
    # No stable public status JSON; treat config / keyring presence lightly.
    home = Path.home()
    if (home / ".grok").is_dir() or (home / ".config" / "grok").is_dir():
        # Directory existing does not guarantee login — still better than lying.
        code, text = _run(["grok", "login", "--help"], timeout=3)
        # Prefer explicit check if we ever get one; for now "may be connected".
        return AccountStatus(ok=None, label="Tap Connect if it asks you to sign in")
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_codex() -> AccountStatus:
    exe = _which_any(("codex",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    code, text = _run([exe, "login", "status"], timeout=6)
    low = text.lower()
    if "logged in" in low or "logged-in" in low or "authenticated" in low:
        return AccountStatus(ok=True, label="Connected")
    if "not logged" in low or "not authenticated" in low or code != 0:
        # codex login status often exits non-zero when logged out.
        if "logged in" in low:
            return AccountStatus(ok=True, label="Connected")
        return AccountStatus(ok=False, label="Needs sign-in")
    return AccountStatus(ok=None, label="Tap Connect if it asks you to sign in")


def _status_copilot() -> AccountStatus:
    exe = _which_any(("copilot",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    # Token files under ~/.copilot when device flow completed.
    home = Path.home()
    copilot_home = Path(os.environ.get("COPILOT_HOME", home / ".copilot"))
    if any(copilot_home.glob("*")):
        return AccountStatus(ok=None, label="Tap Connect if it asks you to sign in")
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_github() -> AccountStatus:
    exe = _which_any(("gh",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    code, text = _run([exe, "auth", "status"], timeout=6)
    if code == 0 and "Logged in" in text:
        # "Logged in to github.com as foo"
        who = ""
        for line in text.splitlines():
            if "Logged in to" in line and " as " in line:
                who = line.split(" as ", 1)[-1].strip()
                break
        label = f"Connected as {who}" if who else "Connected"
        return AccountStatus(ok=True, label=label)
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_perplexity() -> AccountStatus:
    return AccountStatus(ok=True, label="Opens in the browser · sign in there if you want")


def _status_soft() -> AccountStatus:
    return AccountStatus(ok=None, label="Tap Connect to sign in")


_STATUS_PROBES: dict[str, Callable[[], AccountStatus]] = {
    "claude": _status_claude,
    "grok": _status_grok,
    "gemini": _status_soft,
    "codex": _status_codex,
    "copilot": _status_copilot,
    "antigravity": _status_soft,
    "perplexity": _status_perplexity,
    "github": _status_github,
}
