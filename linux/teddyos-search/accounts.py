"""Sign-in catalog for teddyOS helpers — plain language, no terminal required.

Each helper has its own vendor login (browser OAuth almost always). This module
knows *which* command starts that flow and whether the person looks connected,
so a single “Connect” button can do the right thing without anyone learning CLI.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import sqlite3
import subprocess
import tempfile
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
            "We’ll open GitHub Copilot for you. If a code appears, "
            "we copy it and fill the page in — just approve in the browser."
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
        # Web-oriented flags. If GitHub still issues a one-time code, Connect
        # opens the page with the code already filled in (no typing).
        connect_argv=(
            "gh", "auth", "login",
            "--hostname", "github.com",
            "--git-protocol", "https",
            "--web",
        ),
        connect_hint=(
            "We’ll open GitHub for you. Sign in and click Approve — "
            "you shouldn’t need to type any codes."
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


def _looks_signed_out(text: str) -> bool:
    low = (text or "").lower()
    return any(
        s in low
        for s in (
            "not logged in",
            "not logged-in",
            "not authenticated",
            "logged out",
            "no account",
            "please log in",
            "please sign in",
            "sign in required",
            "unauthorized",
        )
    )


def _looks_signed_in(text: str) -> bool:
    """True only for affirmative login — never match inside 'not logged in'."""
    low = (text or "").lower()
    if _looks_signed_out(text):
        return False
    # Prefer clear positive phrases.
    if re.search(r"\blogged in\b", low) and "not " not in low.split("logged in")[0][-8:]:
        return True
    if re.search(r"\bauthenticated\b", low) and "not " not in low.split("authenticated")[0][-8:]:
        return True
    if "login status: logged in" in low or "status: logged in" in low:
        return True
    return False


def _status_codex() -> AccountStatus:
    exe = _which_any(("codex",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    code, text = _run([exe, "login", "status"], timeout=6)
    # "Not logged in" used to match the substring "logged in" — false Connected.
    if _looks_signed_out(text) or (code != 0 and not _looks_signed_in(text)):
        return AccountStatus(ok=False, label="Needs sign-in")
    if _looks_signed_in(text) and code == 0:
        return AccountStatus(ok=True, label="Connected")
    # Auth file present with tokens is a weak positive; status text wins above.
    auth = Path.home() / ".codex" / "auth.json"
    if auth.is_file():
        try:
            data = json.loads(auth.read_text())
            if data.get("OPENAI_API_KEY") or data.get("tokens") or data.get("access_token"):
                # Still only trust if status did not say signed out.
                if not _looks_signed_out(text):
                    return AccountStatus(ok=True, label="Connected")
        except (OSError, json.JSONDecodeError, TypeError):
            pass
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_copilot() -> AccountStatus:
    exe = _which_any(("copilot",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    # Prefer an explicit status command when available.
    code, text = _run([exe, "auth", "status"], timeout=6)
    if text and ("unknown command" not in text.lower() and "usage:" not in text.lower()[:80]):
        if _looks_signed_out(text):
            return AccountStatus(ok=False, label="Needs sign-in")
        if _looks_signed_in(text) and code == 0:
            return AccountStatus(ok=True, label="Connected")
    home = Path.home()
    copilot_home = Path(os.environ.get("COPILOT_HOME", home / ".copilot"))
    # Config dir alone is not proof of login (CLI creates it on first run).
    tokenish = list(copilot_home.glob("**/apps.json")) + list(
        copilot_home.glob("**/*token*")
    )
    if tokenish:
        return AccountStatus(ok=None, label="Tap Connect if it asks you to sign in")
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_github() -> AccountStatus:
    exe = _which_any(("gh",))
    if not exe:
        return AccountStatus(ok=False, label="Not on this computer")
    code, text = _run([exe, "auth", "status"], timeout=6)
    if code == 0 and _looks_signed_in(text):
        who = ""
        for line in text.splitlines():
            if "Logged in to" in line and " as " in line:
                who = line.split(" as ", 1)[-1].strip()
                break
        label = f"Connected as {who}" if who else "Connected"
        return AccountStatus(ok=True, label=label)
    return AccountStatus(ok=False, label="Needs sign-in")


def _status_perplexity() -> AccountStatus:
    """Connected only if the Chromium profile has a real Perplexity session.

    Opening the app once creates a profile and tracking cookies — that is
    not sign-in. Require an auth-ish cookie name, not mere presence of cookies.
    """
    cookies = (
        Path.home() / ".config" / "teddyos-perplexity" / "Default" / "Cookies"
    )
    if not cookies.is_file():
        return AccountStatus(ok=False, label="Needs sign-in")

    try:
        with tempfile.TemporaryDirectory() as tmp:
            copy = Path(tmp) / "Cookies"
            shutil.copy2(cookies, copy)
            for side in ("Cookies-journal", "Cookies-wal", "Cookies-shm"):
                src = cookies.parent / side
                if src.is_file():
                    try:
                        shutil.copy2(src, Path(tmp) / side)
                    except OSError:
                        pass
            con = sqlite3.connect(str(copy))
            try:
                cur = con.execute(
                    "SELECT name FROM cookies "
                    "WHERE host_key LIKE '%perplexity%' LIMIT 80"
                )
                names = {str(n[0]).lower() for n in cur.fetchall()}
            finally:
                con.close()
    except (OSError, sqlite3.Error):
        return AccountStatus(ok=False, label="Needs sign-in")

    if not names:
        return AccountStatus(ok=False, label="Needs sign-in")

    # Anonymous / CDN noise from merely opening the site (not a login).
    def _anonymous(n: str) -> bool:
        if n in {
            "__frs", "__cf_bm", "__cflb", "cf_clearance", "cf_appsession",
            "g_state", "singular_device_id",
            "pplx.edge-sid", "pplx.edge-vid", "pplx.visitor-id",
            "pplx.session-id", "pplx.metadata",
        }:
            return True
        if n.startswith(("_dd", "__cf", "cf_", "pplx.edge", "pplx.visitor")):
            return True
        # Generic "session-id" style analytics, not account auth.
        if n in {"pplx.session-id", "sessionid", "session_id"}:
            return True
        return False

    extras = {n for n in names if not _anonymous(n)}
    # Account-ish names only among non-anonymous cookies.
    strong = {
        n for n in extras
        if any(
            k in n
            for k in (
                "auth", "token", "jwt", "login", "user", "account",
                "next-auth", "stytch", "clerk", "supabase", "oauth",
            )
        )
    }
    if strong:
        return AccountStatus(ok=True, label="Connected")
    return AccountStatus(ok=False, label="Needs sign-in")


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
