"""`kernel/src/ui.rs` `theme::` - parsed, not copied.

The old harness had `ACCENT = (0x00, 0x71, 0xE3)` written out by hand next to a
comment saying it was theme::ACCENT. That is true right up until someone
changes the palette, at which point the harness stops finding the primary
button and the run fails looking like "the button does nothing". Reading the
constants means a palette change is a palette change, not a phantom bug.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
UI_RS = ROOT / "kernel" / "src" / "ui.rs"

_CONST = re.compile(r"pub const (\w+): u32 = 0x([0-9A-Fa-f_]+);")


def load(path: Path = UI_RS) -> dict[str, tuple[int, int, int]]:
    src = path.read_text()
    m = re.search(r"pub mod theme \{(.*?)\n\}", src, re.S)
    if not m:
        raise RuntimeError(f"no `pub mod theme` block in {path}")
    out = {}
    for name, hexval in _CONST.findall(m.group(1)):
        v = int(hexval.replace("_", ""), 16)
        out[name] = ((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF)
    for required in ("BG", "INK", "MUTED", "ACCENT", "RULE", "CARD_BORDER"):
        if required not in out:
            raise RuntimeError(f"theme::{required} vanished from {path}")
    return out


def nav_h(path: Path = UI_RS) -> int:
    """`ui::NAV_H`, used only as a fallback - the nav rule is found on screen."""
    m = re.search(r"pub const NAV_H: i32 = (\d+);", path.read_text())
    return int(m.group(1)) if m else 56


THEME = load()
BG = THEME["BG"]
INK = THEME["INK"]
MUTED = THEME["MUTED"]
ACCENT = THEME["ACCENT"]
RULE = THEME["RULE"]
CARD_BORDER = THEME["CARD_BORDER"]
TINT_BG = THEME.get("TINT_BG")
TINT_BORDER = THEME.get("TINT_BORDER")
ONLINE = THEME.get("ONLINE")
OFFLINE = THEME.get("OFFLINE")
