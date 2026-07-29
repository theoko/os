"""Resolve a 'work on X' goal to a git remote when no local folder exists.

If someone types "i wanna work on tsearch" and there is no ~/tsearch, the useful
next step is not Wikipedia — it is "clone my repo named tsearch". That only
works when the machine can talk to the forge as the user:

  * `gh` signed in (HTTPS token), or
  * SSH keys that github.com accepts, with a git username we can guess

No ambient network for people who never set that up: we probe auth first and
stay quiet when nothing is configured.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

# Where automatic clones land. Next to other projects the person already keeps
# in $HOME, not buried under .cache.
CLONE_ROOT = Path(
    os.environ.get("TEDDYOS_PROJECTS", Path.home() / "Projects")
)


@dataclass(frozen=True)
class RemoteRepo:
    """A forge repo we can clone."""
    full_name: str          # owner/name
    name: str
    clone_url: str          # ssh or https
    description: str = ""
    source: str = ""        # how we found it (gh-search, gh-list, ssh-probe)


@dataclass(frozen=True)
class GitAuth:
    ok: bool
    method: str             # "gh" | "ssh" | "none"
    login: str = ""         # github username when known
    detail: str = ""


def git_auth() -> GitAuth:
    """Whether this machine can clone as the user."""
    if shutil.which("gh"):
        code, out = _run(["gh", "auth", "status"], timeout=8)
        # gh prints to stderr.
        text = out.lower()
        if code == 0 and "logged in" in text:
            login = ""
            m = re.search(r"account\s+(\S+)\s+\(", out, re.I)
            if m:
                login = m.group(1)
            if not login:
                login = _gh_api_login()
            return GitAuth(True, "gh", login=login, detail="gh authenticated")
    # SSH to github without prompting.
    if _ssh_github_ok():
        login = (
            _git_config("github.user")
            or _git_config("github.username")
            or ""
        )
        return GitAuth(
            True, "ssh", login=login,
            detail="SSH to github.com works" + (f" (user {login})" if login else ""),
        )
    return GitAuth(False, "none", detail="no gh login and no working GitHub SSH")


def find_remote_repos(subject: str, limit: int = 8) -> list[RemoteRepo]:
    """Search the user's GitHub for repos matching a work-on subject.

    Returns [] when unauthenticated or nothing matches — never raises.
    """
    slug = (subject or "").strip().strip("/")
    if not slug or len(slug) < 2:
        return []
    auth = git_auth()
    if not auth.ok:
        return []

    # owner/name typed explicitly.
    if re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", slug):
        repo = _resolve_full_name(slug, auth)
        return [repo] if repo else []

    found: list[RemoteRepo] = []
    if auth.method == "gh":
        found.extend(_gh_search(slug, auth.login, limit=limit))
        if len(found) < limit:
            found.extend(_gh_list_match(slug, limit=limit))
    elif auth.method == "ssh" and auth.login:
        found.extend(_ssh_probe_names(slug, auth.login))

    # De-dupe by full_name, prefer earlier (better) hits.
    out: list[RemoteRepo] = []
    seen: set[str] = set()
    for r in found:
        key = r.full_name.lower()
        if key in seen:
            continue
        seen.add(key)
        out.append(r)
        if len(out) >= limit:
            break
    return out


def clone_repo(repo: RemoteRepo, dest_parent: Path | None = None) -> tuple[Path | None, str]:
    """Download into Projects/<name>. Returns (path, technical_message_for_logs).

    The UI turns the path into plain language; the message string is for logs.

    Prefer `gh repo clone` when GitHub CLI is signed in — plain `git clone`
    over HTTPS has no password prompt in our GUI and fails with
    "could not read Username for 'https://github.com'".
    """
    parent = dest_parent or CLONE_ROOT
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        return None, f"mkdir {parent}: {exc}"

    dest = parent / repo.name
    if dest.exists():
        if (dest / ".git").is_dir():
            return dest, f"already at {dest}"
        return None, f"{dest} exists and is not a git checkout"

    auth = git_auth()
    errors: list[str] = []

    # 1) gh repo clone — uses the signed-in account (token in keyring).
    if auth.method == "gh" and shutil.which("gh") and repo.full_name:
        # Ensure git can use gh credentials if anything falls through to git.
        _ensure_gh_git_helper()
        code, out = _run(
            [
                "gh", "repo", "clone", repo.full_name, str(dest),
                "--", "--depth", "1",
            ],
            timeout=300,
        )
        if code == 0 and dest.is_dir():
            return dest, f"cloned {repo.full_name} → {dest} (gh)"
        errors.append(out.strip() or f"gh repo clone exit {code}")
        _cleanup_partial(dest)

    # 2) git clone with the URL we already resolved (SSH or HTTPS).
    if shutil.which("git"):
        url = repo.clone_url or _clone_url(repo.full_name, auth)
        # If we have gh but URL is HTTPS without helper, rewrite via gh.
        if auth.method == "gh" and url.startswith("https://"):
            _ensure_gh_git_helper()
        code, out = _run(
            ["git", "clone", "--depth", "1", url, str(dest)],
            timeout=300,
        )
        if code == 0 and dest.is_dir():
            return dest, f"cloned {repo.full_name} → {dest} (git)"
        errors.append(out.strip() or f"git clone exit {code}")
        _cleanup_partial(dest)

        # 3) Last try: SSH URL if we only attempted HTTPS (or the reverse).
        alt = (
            f"git@github.com:{repo.full_name}.git"
            if url.startswith("https://")
            else f"https://github.com/{repo.full_name}.git"
        )
        if alt != url and repo.full_name:
            code, out = _run(
                ["git", "clone", "--depth", "1", alt, str(dest)],
                timeout=300,
            )
            if code == 0 and dest.is_dir():
                return dest, f"cloned {repo.full_name} → {dest} (git-alt)"
            errors.append(out.strip() or f"git clone alt exit {code}")
            _cleanup_partial(dest)
    else:
        errors.append("git missing")

    return None, " | ".join(e for e in errors if e) or "clone failed"


def _cleanup_partial(dest: Path) -> None:
    if dest.exists():
        try:
            import shutil as sh
            sh.rmtree(dest, ignore_errors=True)
        except Exception:
            pass


def _ensure_gh_git_helper() -> None:
    """Point git at gh so HTTPS clones can use the keyring token."""
    if not shutil.which("gh"):
        return
    # Idempotent; safe to run often.
    _run(["gh", "auth", "setup-git"], timeout=15)


# --- internals --------------------------------------------------------------

def _run(argv: list[str], timeout: float = 30.0) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            argv, capture_output=True, text=True, timeout=timeout,
        )
    except FileNotFoundError:
        return 127, "not found"
    except subprocess.TimeoutExpired:
        return 124, "timed out"
    except OSError as exc:
        return 1, str(exc)
    text = ((completed.stdout or "") + "\n" + (completed.stderr or "")).strip()
    return completed.returncode, text


def _git_config(key: str) -> str:
    code, out = _run(["git", "config", "--get", key], timeout=3)
    return out.strip() if code == 0 else ""


def _ssh_github_ok() -> bool:
    """True when BatchMode ssh to github.com authenticates (exit 1 is success).

    GitHub's shell rejects shells with 'successfully authenticated' and exit 1.
    """
    if not shutil.which("ssh"):
        return False
    # Need at least one key file present; otherwise ssh may hang on password.
    ssh_dir = Path.home() / ".ssh"
    keys = list(ssh_dir.glob("id_*")) if ssh_dir.is_dir() else []
    keys = [k for k in keys if not k.name.endswith(".pub")]
    if not keys:
        return False
    code, out = _run(
        [
            "ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=5", "-T", "git@github.com",
        ],
        timeout=12,
    )
    text = out.lower()
    return "successfully authenticated" in text or "you've successfully authenticated" in text


def _gh_api_login() -> str:
    code, out = _run(["gh", "api", "user", "--jq", ".login"], timeout=8)
    return out.strip() if code == 0 else ""


def _clone_url(full_name: str, auth: GitAuth) -> str:
    # When gh is signed in, HTTPS is fine — clone_repo uses `gh repo clone`
    # (or setup-git). Prefer SSH only when that is the actual auth method.
    if auth.method == "ssh":
        return f"git@github.com:{full_name}.git"
    if auth.method == "gh":
        return f"https://github.com/{full_name}.git"
    if _ssh_github_ok():
        return f"git@github.com:{full_name}.git"
    return f"https://github.com/{full_name}.git"


def _resolve_full_name(full_name: str, auth: GitAuth) -> RemoteRepo | None:
    if auth.method == "gh":
        code, out = _run(
            ["gh", "repo", "view", full_name, "--json", "name,nameWithOwner,description,sshUrl,url"],
            timeout=15,
        )
        if code == 0:
            try:
                data = json.loads(out)
                return RemoteRepo(
                    full_name=data.get("nameWithOwner") or full_name,
                    name=data.get("name") or full_name.split("/")[-1],
                    clone_url=data.get("sshUrl") or _clone_url(full_name, auth),
                    description=(data.get("description") or "")[:120],
                    source="gh-view",
                )
            except (json.JSONDecodeError, TypeError):
                pass
    # SSH probe: does the remote exist?
    url = _clone_url(full_name, auth)
    code, _out = _run(["git", "ls-remote", "--heads", url, "HEAD"], timeout=20)
    if code == 0:
        return RemoteRepo(
            full_name=full_name,
            name=full_name.split("/")[-1],
            clone_url=url,
            source="ssh-probe",
        )
    return None


def _gh_search(slug: str, login: str, limit: int) -> list[RemoteRepo]:
    needle = slug.lower().replace("_", "-")
    queries = []
    if login:
        queries.append(f"{slug} user:{login}")
        queries.append(f"{slug} org:{login}")
    queries.append(slug)
    rows: list = []
    for q in queries:
        code, out = _run(
            [
                "gh", "search", "repos", q,
                "--json", "fullName,name,description,url",
                "--limit", str(limit),
            ],
            timeout=20,
        )
        if code != 0:
            continue
        try:
            batch = json.loads(out)
        except json.JSONDecodeError:
            continue
        if isinstance(batch, list) and batch:
            rows = batch
            break
    if not rows:
        return []
    auth = GitAuth(True, "gh", login=login)
    repos: list[RemoteRepo] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        full = row.get("fullName") or ""
        name = row.get("name") or full.split("/")[-1]
        # Prefer name match over description-only hits.
        if needle not in name.lower().replace("_", "-") and needle not in full.lower():
            continue
        # Prefer the user's own repos when login is known.
        if login and not full.lower().startswith(login.lower() + "/"):
            # Keep as a weaker hit — sort will push own repos first.
            pass
        repos.append(RemoteRepo(
            full_name=full,
            name=name,
            clone_url=_clone_url(full, auth),
            description=(row.get("description") or "")[:120],
            source="gh-search",
        ))
    repos.sort(key=lambda r: (
        0 if login and r.full_name.lower().startswith(login.lower() + "/") else 1,
        0 if needle == r.name.lower().replace("_", "-") else 1,
        0 if needle in r.name.lower() else 1,
        r.full_name.lower(),
    ))
    return repos


def _gh_list_match(slug: str, limit: int) -> list[RemoteRepo]:
    code, out = _run(
        ["gh", "repo", "list", "--limit", "100",
         "--json", "name,nameWithOwner,description,sshUrl,url"],
        timeout=30,
    )
    if code != 0:
        return []
    try:
        rows = json.loads(out)
    except json.JSONDecodeError:
        return []
    needle = slug.lower().replace("_", "-")
    auth = GitAuth(True, "gh")
    hits: list[RemoteRepo] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        name = (row.get("name") or "").lower().replace("_", "-")
        full = row.get("nameWithOwner") or ""
        if needle not in name and needle not in full.lower().replace("_", "-"):
            continue
        hits.append(RemoteRepo(
            full_name=full,
            name=row.get("name") or full.split("/")[-1],
            clone_url=row.get("sshUrl") or _clone_url(full, auth),
            description=(row.get("description") or "")[:120],
            source="gh-list",
        ))
    hits.sort(key=lambda r: (0 if needle == r.name.lower().replace("_", "-") else 1,
                             r.full_name.lower()))
    return hits[:limit]


def _ssh_probe_names(slug: str, login: str) -> list[RemoteRepo]:
    """Without gh search, try a few conventional names under the user's account."""
    base = slug.strip("/").replace(" ", "-")
    candidates = [
        base,
        base.replace("_", "-"),
        f"{base}-revival",
        f"{base}-web",
        f"{base}-app",
    ]
    # Also try if they typed owner/name already handled above.
    out: list[RemoteRepo] = []
    auth = GitAuth(True, "ssh", login=login)
    for name in candidates:
        full = f"{login}/{name}"
        repo = _resolve_full_name(full, auth)
        if repo:
            out.append(repo)
    return out
