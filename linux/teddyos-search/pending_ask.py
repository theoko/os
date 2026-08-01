"""One pending question after first-time sign-in.

A new user types what they need, we send them through sign-in, then they
forget the question or the project. This file remembers it so Search can
put them back where they were — without asking them to invent any app.

Kinds:
  work      — project folder help (“work on trading”)
  freeform  — everyday task against home (email-ish asks)
  linkedin  — AI draft replies for LinkedIn (re-run original Search query)
  whatsapp  — AI draft replies for WhatsApp (re-run original Search query)

Never store the long internal model playbook as the only restore key for
messaging — re-run the human words they typed so freeform mode rebuilds.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

_PATH = Path(
    os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")
) / "teddyos" / "pending-ask.json"

_KINDS = frozenset({"work", "freeform", "linkedin", "whatsapp"})


def save(
    project: str,
    prompt: str = "",
    *,
    query: str = "",
    kind: str = "work",
) -> None:
    """Remember enough to restore after sign-in.

    `query` is what they typed in Search (preferred for freeform/messaging).
    `prompt` is an optional extra (leftover task words for project work).
    """
    project = (project or "").strip()
    prompt = (prompt or "").strip()
    query = (query or "").strip()
    kind = (kind or "work").strip().lower()
    if kind not in _KINDS:
        kind = "work"
    if not project and not query:
        return
    try:
        _PATH.parent.mkdir(parents=True, exist_ok=True)
        _PATH.write_text(json.dumps({
            "project": project,
            "prompt": prompt,
            "query": query,
            "kind": kind,
        }, indent=2) + "\n")
    except OSError:
        pass


def load() -> dict | None:
    try:
        data = json.loads(_PATH.read_text())
    except (OSError, json.JSONDecodeError, TypeError):
        return None
    if not isinstance(data, dict):
        return None
    return data


def clear() -> None:
    try:
        _PATH.unlink(missing_ok=True)
    except OSError:
        pass
