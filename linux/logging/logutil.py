"""Shared logging for teddyOS apps.

Every app used to print to a terminal nobody was looking at, or to nowhere at
all. When Search froze or setup failed on first boot, the only evidence was
whatever still happened to be in the journal — if the journal was still in
memory and the machine had not rebooted.

This module writes two places at once:

  1. /var/log/teddyos/app/<name>.log  (rotating files, survives reboot)
  2. the system journal via `logger -t teddyos-<name>` (queryable, centralized)

If /var/log/teddyos is not writable (live session before the log dir exists),
it falls back to ~/.local/state/teddyos/log/ so a non-root app still leaves a
trail. Failures here never raise — logging must not be how an app crashes.
"""

from __future__ import annotations

import logging
import os
import subprocess
from logging.handlers import RotatingFileHandler
from pathlib import Path

SYSTEM_LOG_DIR = Path("/var/log/teddyos/app")
USER_LOG_DIR = Path(
    os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state")
) / "teddyos" / "log"

# 2 MB × 5 files per app ≈ 10 MB ceiling. Enough for days of chatter without
# eating the disk on a 32 GB laptop image.
MAX_BYTES = 2 * 1024 * 1024
BACKUP_COUNT = 5

_configured: set[str] = set()


def get_logger(name: str) -> logging.Logger:
    """Return a named logger, configured once per process."""
    key = name.strip() or "teddyos"
    log = logging.getLogger(f"teddyos.{key}")
    if key in _configured:
        return log
    _configured.add(key)
    log.setLevel(logging.DEBUG)
    log.propagate = False

    fmt = logging.Formatter(
        "%(asctime)s %(levelname)s %(name)s: %(message)s",
        datefmt="%Y-%m-%dT%H:%M:%S",
    )

    path = _log_path(key)
    if path is not None:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            fh = RotatingFileHandler(
                path, maxBytes=MAX_BYTES, backupCount=BACKUP_COUNT,
                encoding="utf-8",
            )
            fh.setLevel(logging.DEBUG)
            fh.setFormatter(fmt)
            log.addHandler(fh)
        except OSError:
            pass

    # Journal / syslog. logger(1) is on every Debian image we ship.
    log.addHandler(_JournalHandler(f"teddyos-{key}"))

    # stderr last so an interactive terminal still sees what happened.
    sh = logging.StreamHandler()
    sh.setLevel(logging.WARNING)
    sh.setFormatter(fmt)
    log.addHandler(sh)
    return log


def _log_path(name: str) -> Path | None:
    for base in (SYSTEM_LOG_DIR, USER_LOG_DIR):
        try:
            base.mkdir(parents=True, exist_ok=True)
            # Probe writability with the real target name.
            candidate = base / f"{name}.log"
            if base == SYSTEM_LOG_DIR and not os.access(base, os.W_OK):
                continue
            return candidate
        except OSError:
            continue
    return None


class _JournalHandler(logging.Handler):
    """Ship a line to the journal without depending on python3-systemd."""

    def __init__(self, tag: str):
        super().__init__(level=logging.INFO)
        self.tag = tag

    def emit(self, record: logging.LogRecord) -> None:
        try:
            msg = self.format(record) if self.formatter else record.getMessage()
            # Keep lines short — logger can drop huge blobs.
            if len(msg) > 1800:
                msg = msg[:1800] + "…"
            subprocess.run(
                ["logger", "-t", self.tag, "--", msg],
                check=False,
                capture_output=True,
                timeout=2,
            )
        except Exception:
            self.handleError(record)
