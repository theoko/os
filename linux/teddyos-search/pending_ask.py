"""One pending question after first-time sign-in.

A new user types what they need, we send them through sign-in, then they
forget the question or the project. This file remembers it so Search can
put them back where they were — without asking them to “open helpers.”
"""

from __future__ import annotations

import json
import os
from pathlib import Path

_PATH = Path(
    os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")
) / "teddyos" / "pending-ask.json"


def save(project: str, prompt: str) -> None:
    prompt = (prompt or "").strip()
    project = (project or "").strip()
    if not project:
        return
    try:
        _PATH.parent.mkdir(parents=True, exist_ok=True)
        _PATH.write_text(json.dumps({
            "project": project,
            "prompt": prompt,
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
