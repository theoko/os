#!/usr/bin/env python3
"""End-to-end invariants for the real ISO, asserted against the real screen.

`scripts/drive-ui.py` walks a *script*: click Continue six times, click the
pixel where the portal pill used to be, type into the coordinates the search
field had last month. Every one of those is a claim about a UI that another
session is redesigning continuously, and every time one went stale the run
failed in a way that looked like a product bug. The placeholder changed, Enter
started running an agent Brief instead of a search, a Status screen and a Brief
screen appeared, the capability rows moved twice, setup got shorter. The
harness broke on all of it and blamed the product each time.

So this file does not know where anything is. It derives the current state from
the screen — the nav title strip says which view you are on, the accent pill
says where the primary action is, the border colour says where the rows are,
and macOS's own text recogniser says what the words are — decides the next
action from that, and reads the screen back to confirm it happened.

What it asserts, and the bug it exists to catch:

  1. control drawn but not wired ......... every control clicked must do
                                          something observable
  2. control off-screen or overlapping ... nothing drawn may be cut off by the
                                          edge, and no two controls may
                                          partially overlap
  3. prose contradicting the render ...... a count stated in words must equal
                                          the rows drawn beneath it
  4. dropped keystrokes .................. what the bridge received must be
                                          exactly what was typed
  5. sentence naming the wrong source .... "offline" above rows the bridge
                                          served is a lie
  6. the same explanation twice .......... two wordings of one sentence on one
                                          screen
  7. duplicate results ................... one title must not appear twice in
                                          one list (the live arm64 Doc/Hit bug)
  8. the field restarting setup .......... clicking the search field must leave
                                          you on the home screen

Every assertion names what it checks in plain language and, on failure, prints
what was expected, what was seen, and the screenshot it was seen on.

    ./scripts/e2e/invariants.py                 # full run, then self-proof
    ./scripts/e2e/invariants.py --out /tmp/e2e  # where the evidence lands
    ./scripts/e2e/invariants.py --no-prove      # skip the self-proof
    make e2e                                    # the same thing, from the top

A test that cannot fail is worse than none, and this project has shipped
those. `--prove` (on by default) therefore takes the evidence this run actually
collected, breaks one thing in it per assertion, and requires that assertion to
go red. An assertion that stays green on deliberately broken evidence is itself
reported as a failure.

Self-contained on purpose: another session is building `theme.py`, `atlas.py`,
`catalog.py`, `frame.py` and `screen.py` in this directory. Importing a module
that is being written underneath a running test turns someone else's edit into
this suite's failure, which is the exact confusion the suite exists to end.
Merge the two deliberately later; do not couple them by accident.
"""

from __future__ import annotations

import argparse
import copy
import json
import re
import shutil
import socket
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

try:
    import numpy as np
except ImportError:  # pragma: no cover
    raise SystemExit(
        "invariants.py needs numpy to scan a 1280x800 framebuffer at a useful "
        "speed: python3 -m pip install numpy"
    )

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

# ---------------------------------------------------------------------------
# palette
# ---------------------------------------------------------------------------
# Read out of the kernel, never copied. The old harness had `ACCENT = (0x00,
# 0x71, 0xE3)` written by hand beside a comment saying it was theme::ACCENT.
# That is true until someone changes the palette, at which point the harness
# stops finding the primary button and the run fails looking like "the button
# does nothing".

_UI_RS = ROOT / "kernel" / "src" / "ui.rs"
_CONST = re.compile(r"pub const (\w+): u32 = 0x([0-9A-Fa-f_]+);")


def load_theme(path: Path = _UI_RS) -> dict[str, tuple[int, int, int]]:
    src = path.read_text()
    m = re.search(r"pub mod theme \{(.*?)\n\}", src, re.S)
    if not m:
        raise SystemExit(f"no `pub mod theme` block in {path} — the UI moved, not the test")
    out = {}
    for name, hexval in _CONST.findall(m.group(1)):
        v = int(hexval.replace("_", ""), 16)
        out[name] = ((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF)
    for need in ("BG", "ACCENT", "RULE", "CARD_BORDER"):
        if need not in out:
            raise SystemExit(f"theme::{need} vanished from {path}")
    return out


THEME = load_theme()

# Colours that outline or fill a control. Text is drawn in INK/MUTED and is
# excluded: a letter stem is a vertical run too, just a short one, and the
# height floor below is what keeps glyphs out of the control list.
def structural_colours() -> list[tuple[int, int, int]]:
    keys = ["CARD_BORDER", "RULE", "ACCENT", "TINT_BORDER"]
    return [THEME[k] for k in keys if k in THEME]


# A control has to be at least this tall and wide to be one. 18px is above the
# tallest glyph stem in the atlas and below the shortest real control (the
# 28px setup toggle track).
MIN_CTRL_H = 18
MIN_CTRL_W = 36


# ---------------------------------------------------------------------------
# geometry
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    w: int
    h: int

    @property
    def right(self) -> int:
        return self.x + self.w

    @property
    def bottom(self) -> int:
        return self.y + self.h

    @property
    def centre(self) -> tuple[int, int]:
        return (self.x + self.w // 2, self.y + self.h // 2)

    def contains_point(self, px: int, py: int) -> bool:
        return self.x <= px < self.right and self.y <= py < self.bottom

    def contains(self, o: "Rect") -> bool:
        return self.x <= o.x and self.y <= o.y and self.right >= o.right and self.bottom >= o.bottom

    def intersection(self, o: "Rect") -> "Rect | None":
        x0, y0 = max(self.x, o.x), max(self.y, o.y)
        x1, y1 = min(self.right, o.right), min(self.bottom, o.bottom)
        if x1 <= x0 or y1 <= y0:
            return None
        return Rect(x0, y0, x1 - x0, y1 - y0)

    def __repr__(self) -> str:
        return f"({self.x},{self.y} {self.w}x{self.h})"


@dataclass
class Word:
    """One line of recognised text and where it sits, in image pixels."""

    text: str
    conf: float
    box: Rect

    def __repr__(self) -> str:
        return f"{self.box}{self.text!r}"


# ---------------------------------------------------------------------------
# reading a screendump
# ---------------------------------------------------------------------------


def read_ppm(path: Path) -> tuple[int, int, bytes]:
    """QEMU writes binary P6: ASCII header, then raw RGB triples."""
    data = path.read_bytes()
    if data[:2] != b"P6":
        raise RuntimeError(f"{path} is not a binary PPM ({data[:8]!r})")
    fields, pos = [], 2
    while len(fields) < 3:
        while pos < len(data) and data[pos : pos + 1].isspace():
            pos += 1
        if data[pos : pos + 1] == b"#":
            while data[pos : pos + 1] not in (b"\n", b""):
                pos += 1
            continue
        start = pos
        while pos < len(data) and not data[pos : pos + 1].isspace():
            pos += 1
        fields.append(int(data[start:pos]))
    pos += 1  # the single whitespace after maxval
    w, h, _ = fields
    img = np.frombuffer(data, dtype=np.uint8, count=w * h * 3, offset=pos)
    return w, h, img.reshape(h, w, 3)


_OCR_SRC = HERE / "screentext.swift"
_OCR_BIN = HERE / ".screentext"


def ocr_binary() -> Path:
    """Compile the recognizer once, then reuse it."""
    if _OCR_BIN.exists() and _OCR_BIN.stat().st_mtime >= _OCR_SRC.stat().st_mtime:
        return _OCR_BIN
    r = subprocess.run(
        ["swiftc", "-O", str(_OCR_SRC), "-o", str(_OCR_BIN)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise SystemExit(f"cannot build {_OCR_SRC.name}:\n{r.stdout}\n{r.stderr}")
    return _OCR_BIN


def read_words(png: Path) -> list[Word]:
    out = subprocess.run(
        [str(ocr_binary()), str(png)], capture_output=True, check=True
    ).stdout
    doc = json.loads(out)
    return [
        Word(
            text=l["text"],
            conf=float(l["conf"]),
            box=Rect(int(l["x"]), int(l["y"]), max(1, int(l["w"])), max(1, int(l["h"]))),
        )
        for l in doc["lines"]
    ]


class Shot:
    """One screendump: its pixels, its words, and what can be read off them.

    Nothing in here knows where a control is *supposed* to be. That is the
    whole point: the previous harness did, and every redesign made it lie.
    """

    def __init__(self, name: str, ppm: Path, png: Path):
        self.name = name
        self.ppm = ppm
        self.png = png
        self.w, self.h, self.px = read_ppm(ppm)
        self.words: list[Word] = read_words(png)
        self._controls: list[Rect] | None = None

    # -- pixels ------------------------------------------------------------

    def at(self, x: int, y: int) -> tuple[int, int, int]:
        return tuple(int(v) for v in self.px[y, x])

    def mask(self, colours, tol: int = 8):
        """Which pixels are one of `colours`, within `tol` per channel."""
        img = self.px.astype(np.int16)
        out = np.zeros(img.shape[:2], dtype=bool)
        for c in colours:
            out |= (np.abs(img - np.array(c, dtype=np.int16)) <= tol).all(axis=2)
        return out

    # -- controls ----------------------------------------------------------

    @property
    def controls(self) -> list[Rect]:
        """Every control-shaped thing actually drawn on this screen.

        Found the way a person finds one: a shape whose edges are one of the
        palette's structural colours. Both the outlined kind (a card: two 1px
        vertical edges the same height) and the filled kind (a pill: a solid
        block) fall out of the same scan, because both give columns of
        structural pixels with the same top and bottom.
        """
        if self._controls is None:
            self._controls = self._detect_controls()
        return self._controls

    def _detect_controls(self, tol: int = 8) -> list[Rect]:
        w, h = self.w, self.h
        m = self.mask(structural_colours(), tol).astype(np.int8)

        # Vertical runs, column by column. A card's left and right borders are
        # 1px columns of CARD_BORDER running the height of the card; a filled
        # pill is a solid block of ACCENT columns. Both are runs, so one scan
        # finds the outlined controls and the filled ones together.
        pad = np.zeros((1, w), dtype=np.int8)
        d = np.diff(np.vstack([pad, m, pad]), axis=0)
        sy, sx = np.nonzero(d == 1)
        ey, ex = np.nonzero(d == -1)
        so = np.lexsort((sy, sx))
        eo = np.lexsort((ey, ex))
        sy, sx, ey = sy[so], sx[so], ey[eo]
        keep = (ey - sy) >= MIN_CTRL_H
        runs: dict[tuple[int, int], list[int]] = {}
        for x, y0, y1 in zip(sx[keep].tolist(), sy[keep].tolist(), ey[keep].tolist()):
            runs.setdefault((y0, y1), []).append(x)

        rects: list[Rect] = []
        for (y0, y1), xs in runs.items():
            xs.sort()
            # Split the columns that share this exact top and bottom into the
            # separate things they belong to. Three home tiles share a top and
            # a bottom; they are three controls, not one.
            clusters: list[list[int]] = [[xs[0]]]
            for x in xs[1:]:
                if x - clusters[-1][-1] > 8:
                    clusters.append([x])
                else:
                    clusters[-1].append(x)
            spans = [(c[0], c[-1] + 1) for c in clusters]
            narrow = [s for s in spans if s[1] - s[0] <= 6]
            wide = [s for s in spans if s[1] - s[0] > 6]
            for x0, x1 in wide:
                if x1 - x0 >= MIN_CTRL_W:
                    rects.append(Rect(x0, y0, x1 - x0, y1 - y0))
            # Two thin edges the same height, facing each other, are the left
            # and right sides of one outlined control. An unmatched edge is
            # left out rather than guessed at: inventing the missing side would
            # invent a control, and a harness that invents controls reports
            # phantom bugs, which is the failure mode this file exists to end.
            for a, b in zip(narrow[0::2], narrow[1::2]):
                if b[1] - a[0] >= MIN_CTRL_W:
                    rects.append(Rect(a[0], y0, b[1] - a[0], y1 - y0))

        # A rounded pill is not one run: its corners are shorter than its body,
        # so one control arrives as a handful of overlapping fragments. Fold
        # anything that mostly sits inside something else back into it.
        rects.sort(key=lambda r: r.w * r.h, reverse=True)
        merged: list[Rect] = []
        for r in rects:
            for i, m in enumerate(merged):
                inter = m.intersection(r)
                if inter and inter.w * inter.h * 2 >= min(m.w * m.h, r.w * r.h):
                    merged[i] = Rect(
                        min(m.x, r.x),
                        min(m.y, r.y),
                        max(m.right, r.right) - min(m.x, r.x),
                        max(m.bottom, r.bottom) - min(m.y, r.y),
                    )
                    break
            else:
                merged.append(r)
        merged.sort(key=lambda r: (r.y, r.x))
        return merged

    # -- what kind of control -------------------------------------------

    def border_kind(self, r: Rect) -> str:
        """Which palette colour outlines this control.

        The kernel draws the query field in `theme::RULE`, a result row or a
        tile in `theme::CARD_BORDER`, and a live/selected row in
        `theme::ACCENT`. That distinction is on the screen, so the harness
        reads it there instead of deciding by position — which is how the
        previous one mistook the first Skills row for the search box.
        """
        ys = range(max(0, r.y + r.h // 4), min(self.h, r.bottom - r.h // 4)) or [r.centre[1]]
        xs = [r.x, r.x + 1, r.right - 1, r.right - 2]
        tally: dict[str, int] = {}
        for x in xs:
            if not 0 <= x < self.w:
                continue
            for y in ys:
                c = self.at(x, y)
                for name in ("RULE", "CARD_BORDER", "ACCENT", "TINT_BORDER"):
                    if name in THEME and all(abs(c[k] - THEME[name][k]) <= 8 for k in range(3)):
                        tally[name] = tally.get(name, 0) + 1
                        break
        if not tally:
            return "none"
        return max(tally.items(), key=lambda kv: kv[1])[0]

    def is_filled(self, r: Rect, name: str = "ACCENT") -> bool:
        return name in THEME and self._fill_fraction(r, THEME[name]) >= 0.55

    def primary_action(self) -> Rect | None:
        """The one filled block of accent: Continue, Start, the primary CTA.

        Width alone picked the wrong control — a selected row is *outlined* in
        accent and three times wider than the pill. Requiring the block to be
        filled is what separates a pill from a frame.
        """
        best = None
        for r in self.controls:
            if r.w < 90 or r.h < 20 or r.h > 90:
                continue
            if not self.is_filled(r, "ACCENT"):
                continue
            if best is None or r.w * r.h > best.w * best.h:
                best = r
        return best

    def toggle_on(self, row: Rect) -> bool | None:
        """Is this row's switch on? None when the row has no switch.

        `setup::row` paints the track in ACCENT when the capability is granted
        and in RULE when it is not. Reading that is the difference between
        granting a capability and un-granting one: the first run of this suite
        "granted Built-in docs" by clicking a switch that was already on, and
        the serial log came back saying only Your files was enabled. The screen
        had been saying so the whole time.
        """
        x0 = max(0, row.right - 70)
        patch = self.px[row.y : row.bottom, x0 : row.right].astype(np.int16)
        if patch.size == 0:
            return None
        on = float((np.abs(patch - np.array(THEME["ACCENT"], np.int16)) <= 8).all(axis=2).mean())
        off = float((np.abs(patch - np.array(THEME["RULE"], np.int16)) <= 8).all(axis=2).mean())
        if max(on, off) < 0.05:
            return None
        return on > off

    def _fill_fraction(self, r: Rect, colour, tol: int = 8) -> float:
        y0, y1 = max(0, r.y + 2), min(self.h, r.bottom - 2)
        x0, x1 = max(0, r.x + 2), min(self.w, r.right - 2)
        if y1 <= y0 or x1 <= x0:
            return 0.0
        patch = self.px[y0:y1, x0:x1].astype(np.int16)
        hit = (np.abs(patch - np.array(colour, dtype=np.int16)) <= tol).all(axis=2)
        return float(hit.mean())

    def nav_rule_y(self) -> int:
        """Where the nav hairline actually is, rather than where NAV_H says.

        `ui::paint_chill_rule` breathes this line between RULE and ACCENT on a
        60 Hz timer, so the colour is whatever the animation happened to be on
        when the screendump landed. What does not change is that it is the one
        full-width horizontal line near the top, so that is what is looked for.
        """
        band = self.px[: min(120, self.h)].astype(np.int16)
        bg = np.array(THEME["BG"], dtype=np.int16)
        non_bg = (np.abs(band - bg) > 10).any(axis=2)
        for y in range(10, band.shape[0]):
            if non_bg[y].mean() >= 0.95:
                return y
        return 56

    # -- words -------------------------------------------------------------

    @property
    def text(self) -> str:
        return " ".join(word.text for word in self.words)

    def words_in(self, r: Rect) -> list[Word]:
        out = []
        for word in self.words:
            inter = r.intersection(word.box)
            if inter and inter.w * inter.h * 2 >= word.box.w * word.box.h:
                out.append(word)
        return out

    def find(self, needle: str) -> Word | None:
        n = needle.lower()
        for word in self.words:
            if n in word.text.lower():
                return word
        return None

    def nav_title(self) -> str | None:
        """The centred word in the nav strip: the guest saying where you are."""
        rule = self.nav_rule_y()
        best = None
        for word in self.words:
            if word.box.bottom > rule + 4:
                continue
            mid = word.box.x + word.box.w // 2
            if abs(mid - self.w // 2) > 70:
                continue
            if best is None or word.box.w > best.box.w:
                best = word
        return best.text.strip() if best else None

    # -- state -------------------------------------------------------------

    def view(self, titles: set[str]) -> str:
        """Which screen this is, derived from the screen and nothing else."""
        title = self.nav_title()
        if title:
            for known in titles:
                if title.lower() == known.lower():
                    return known.lower()
        # No nav title: either home, which draws the brand at the left instead
        # of a centred title, or a setup step, which draws neither. Home has a
        # query field and no primary pill; setup has a pill and no field.
        pill = self.primary_action()
        field = self.text_field()
        if field is not None and pill is None:
            return "home"
        if pill is not None:
            return "setup"
        if title:
            return f"unknown:{title}"
        return "unknown"

    def text_field(self) -> Rect | None:
        """The query box: the wide RULE-outlined box below the nav.

        Found by its outline colour, not its coordinates. Its placeholder has
        changed twice ("Search the knowledge base" -> "What do you want to work
        on?") and its y once, and both changes broke a harness that looked for
        it by position or by its words.
        """
        rule_y = self.nav_rule_y()
        cands = [
            r
            for r in self.controls
            if r.y > rule_y
            and r.y < self.h // 2
            and r.w >= self.w // 2
            and 20 <= r.h <= 70
            and self.border_kind(r) == "RULE"
            and self._fill_fraction(r, THEME["BG"], tol=6) > 0.7
        ]
        return min(cands, key=lambda r: r.y) if cands else None

    def rows(self) -> list[Rect]:
        """Row-shaped controls: the list items a screen renders.

        A card, a result row and a capability row are all drawn as a
        CARD_BORDER (or, when live, ACCENT) outline around background. The
        primary pill is a *filled* accent block and is not a row.
        """
        rule_y = self.nav_rule_y()
        out = [
            r
            for r in self.controls
            if r.y > rule_y
            and r.w >= self.w // 3
            and 20 <= r.h <= 110
            and self.border_kind(r) in ("CARD_BORDER", "ACCENT", "TINT_BORDER")
            and not self.is_filled(r, "ACCENT")
        ]
        out.sort(key=lambda r: r.y)
        return out


# ---------------------------------------------------------------------------
# assertions
# ---------------------------------------------------------------------------


class Checks:
    """Every assertion, reported together.

    One failure must not hide the rest — a run that stops at the first red
    tells you one thing about a screen that had four things wrong with it.
    """

    def __init__(self, quiet: bool = False):
        self.results: list[tuple[bool, str, str, str]] = []
        self.quiet = quiet

    def that(self, name: str, ok, expected: str = "", seen: str = "", shot: str = "") -> bool:
        ok = bool(ok)
        detail = ""
        if not ok:
            detail = f"expected {expected}; saw {seen}"
            if shot:
                detail += f"; screenshot {shot}"
        self.results.append((ok, name, detail, shot))
        if not self.quiet:
            print(f"  {'PASS' if ok else 'FAIL'}  {name}")
            if not ok:
                print(f"          expected: {expected}")
                print(f"          saw:      {seen}")
                if shot:
                    print(f"          shot:     {shot}")
        return ok

    def note(self, text: str) -> None:
        if not self.quiet:
            print(f"  ....  {text}")

    @property
    def failed(self) -> list[tuple[bool, str, str, str]]:
        return [r for r in self.results if not r[0]]

    def report(self) -> int:
        print(f"\n{len(self.results) - len(self.failed)}/{len(self.results)} assertions passed")
        for _, name, detail, _ in self.failed:
            print(f"  FAILED: {name}")
            print(f"          {detail}")
        return 1 if self.failed else 0


# ---------------------------------------------------------------------------
# the observation this run collected
# ---------------------------------------------------------------------------


@dataclass
class Observation:
    """Everything the run saw. Assertions are pure functions of this.

    Keeping the evidence separate from the driving is what makes `--prove`
    possible: the same assertion can be run against the real observation and
    against a deliberately damaged copy of it, and it has to disagree.
    """

    serial: str = ""
    bridge: str = ""
    shots: list[Shot] = field(default_factory=list)
    typed: str = ""
    typed_shot: Shot | None = None
    received: str | None = None
    result_shots: list[Shot] = field(default_factory=list)
    field_click_view: str | None = None
    field_click_shot: Shot | None = None
    wired: list[tuple[str, bool, str, str]] = field(default_factory=list)
    setup_steps_without_action: list[str] = field(default_factory=list)
    reached_home: bool = False
    # True when the host bridge was replaced under the running guest — another
    # session's `ensure-bridge.sh`, usually. It makes the guest look mute when
    # it is not, so it is called out rather than blamed on the product.
    bridge_restarted: bool = False
    swept: bool = True


# -- the invariants ---------------------------------------------------------


def check_no_fault(obs: Observation, ck: Checks) -> None:
    bad = [l for l in obs.serial.splitlines() if "FAULT" in l]
    ck.that(
        "the kernel never reported a CPU fault",
        not bad,
        expected="no line containing FAULT on the serial log",
        seen=f"{len(bad)} line(s): {bad[:2]}",
    )


def check_no_panic(obs: Observation, ck: Checks) -> None:
    bad = [l for l in obs.serial.splitlines() if "PANIC" in l or "panicked" in l]
    ck.that(
        "the kernel never panicked",
        not bad,
        expected="no line containing PANIC on the serial log",
        seen=f"{len(bad)} line(s): {bad[:2]}",
    )


def check_controls_on_screen(obs: Observation, ck: Checks) -> None:
    """Bug class 2, half of it: a control drawn past the edge cannot be used."""
    offenders = []
    for s in obs.shots:
        for r in s.controls:
            if r.x <= 0 or r.y <= 0 or r.right >= s.w or r.bottom >= s.h:
                offenders.append((s, r))
    first = offenders[0] if offenders else None
    ck.that(
        "no control is cut off by the edge of the screen",
        not offenders,
        expected="every drawn control fully inside the framebuffer",
        seen=(
            f"{len(offenders)} clipped, first {first[1]} on {first[0].name} "
            f"({first[0].w}x{first[0].h})"
            if first
            else "none"
        ),
        shot=str(first[0].png) if first else "",
    )


def check_controls_do_not_overlap(obs: Observation, ck: Checks) -> None:
    """Bug class 2, the other half: two controls fighting over the same pixels.

    Nesting is not overlap — a pill inside a card is a pill inside a card. The
    bug is a *partial* overlap, where clicking the shared strip routes to
    whichever zone was registered first and the other control is unreachable.
    """
    offenders = []
    for s in obs.shots:
        ctrls = s.controls
        for i in range(len(ctrls)):
            for j in range(i + 1, len(ctrls)):
                a, b = ctrls[i], ctrls[j]
                if a.contains(b) or b.contains(a):
                    continue
                inter = a.intersection(b)
                if inter and inter.w > 3 and inter.h > 3:
                    offenders.append((s, a, b, inter))
    first = offenders[0] if offenders else None
    ck.that(
        "no two controls on a screen partially overlap",
        not offenders,
        expected="controls are nested or disjoint, never half on top of each other",
        seen=(
            f"{len(offenders)} pair(s), first {first[1]} vs {first[2]} "
            f"sharing {first[3]} on {first[0].name}"
            if first
            else "none"
        ),
        shot=str(first[0].png) if first else "",
    )


def check_text_not_clipped(obs: Observation, ck: Checks) -> None:
    offenders = []
    for s in obs.shots:
        for word in s.words:
            b = word.box
            if b.x <= 0 or b.y <= 0 or b.right >= s.w or b.bottom >= s.h:
                offenders.append((s, word))
    first = offenders[0] if offenders else None
    ck.that(
        "no text runs off the edge of the screen",
        not offenders,
        expected="every recognised line fully inside the framebuffer",
        seen=(
            f"{len(offenders)} clipped, first {first[1].text!r} at {first[1].box} on {first[0].name}"
            if first
            else "none"
        ),
        shot=str(first[0].png) if first else "",
    )


def check_setup_always_offers_a_way_forward(obs: Observation, ck: Checks) -> None:
    """Bug class 2 as it actually bites: the primary action below the fold.

    A Continue pill drawn past the bottom edge is not a cosmetic problem, it is
    a machine you cannot finish setting up.
    """
    ck.that(
        "every setup step offers a primary action that is on screen",
        not obs.setup_steps_without_action,
        expected="an accent pill found on every setup step",
        seen=f"no reachable primary action on {obs.setup_steps_without_action}",
        shot=obs.setup_steps_without_action[0] if obs.setup_steps_without_action else "",
    )


def check_home_was_reached(obs: Observation, ck: Checks) -> None:
    ck.that(
        "the setup journey ends on the home screen",
        obs.reached_home,
        expected="the derived state to become 'home' within the step budget",
        seen="the journey never reached home — see the numbered shots",
        shot=str(obs.shots[-1].png) if obs.shots else "",
    )


_ALNUM = re.compile(r"[^a-z0-9 ]+")


def normalise(s: str) -> str:
    return " ".join(_ALNUM.sub(" ", s.lower()).split())


# Tags the Brief prefixes its rows with. They label a row, they are not part of
# its identity — which is exactly why "Doc: x" and "Hit: x" are the same row.
ROW_TAGS = {
    "doc",
    "hit",
    "goal",
    "query",
    "info",
    "next",
    "fyi",
    "urgent",
    "reply",
    "need",
    "event",
    "note",
}


# Tags that name a finding. Only these can be "the same row twice"; the rest
# describe the run, and two of them may legitimately carry the same words. A
# one-word goal prints "Goal: opportunistic" and "Query: opportunistic" — two
# facts about one word, not one document listed twice. The kernel draws the
# same line and deduplicates results only (`Brief::is_result`).
RESULT_TAGS = {"doc", "hit"}


def row_tag(text: str) -> str:
    words = normalise(text).split()
    return words[0] if words and words[0] in ROW_TAGS else ""


def row_identity(text: str) -> str:
    words = normalise(text).split()
    while words and words[0] in ROW_TAGS:
        words = words[1:]
    return " ".join(words)


def check_no_duplicate_rows(obs: Observation, ck: Checks) -> None:
    """Bug class 7. Live on arm64 right now: one document, listed twice.

    `agent.rs` pushes a `Doc` row per intent hit, then — when none of those
    hits carried a URL, so `doc_n` never moved — pushes the same titles again
    as `Hit`. Two labels, one document, and the count above them says two.
    """
    if not obs.result_shots:
        ck.that(
            "no result list shows the same row twice",
            False,
            expected="a result screen to assert against",
            seen="the run never produced one",
        )
        return
    dupes = []
    total = 0
    for s in obs.result_shots:
        seen: dict[str, str] = {}
        for r in s.rows():
            body = " ".join(w.text for w in s.words_in(r))
            ident = row_identity(body)
            if not ident or row_tag(body) not in RESULT_TAGS:
                continue
            if ident in seen:
                dupes.append((s, seen[ident], body))
            else:
                seen[ident] = body
                total += 1
    ck.that(
        "no result list shows the same row twice",
        not dupes,
        expected="every row on a result screen to name a different thing",
        seen=(
            f"{dupes[0][1]!r} and {dupes[0][2]!r} are the same row on {dupes[0][0].name}"
            if dupes
            else f"{total} distinct rows across {len(obs.result_shots)} result screen(s)"
        ),
        shot=str(dupes[0][0].png) if dupes else "",
    )


_COUNT = re.compile(
    r"\b(\d+)\s+(matches|match|results|result|hits|hit|docs|doc|rows|row|"
    r"messages|message|files|file|items|item)\b"
)


def check_prose_count_matches_rows(obs: Observation, ck: Checks) -> None:
    """Bug class 3: "5 matches for nvda." printed above three cards."""
    if not obs.result_shots:
        ck.that(
            "a count stated in prose equals the rows rendered",
            False,
            expected="a result screen to assert against",
            seen="the run never produced one",
        )
        return
    wrong, claims = [], 0
    for s in obs.result_shots:
        drawn = len(s.rows())
        for word in s.words:
            for m in _COUNT.finditer(word.text.lower()):
                claims += 1
                if int(m.group(1)) != drawn:
                    wrong.append((s, word.text.strip(), int(m.group(1)), drawn))
    if not claims:
        ck.note(
            "no count stated in prose on any result screen this run — "
            "nothing to contradict"
        )
    ck.that(
        "a count stated in prose equals the rows rendered",
        not wrong,
        expected="every count printed in words to equal the rows drawn beneath it",
        seen=(
            f"{wrong[0][1]!r} on {wrong[0][0].name} claims {wrong[0][2]} above {wrong[0][3]} row(s)"
            if wrong
            else f"{claims} count(s) agree with what is drawn"
        ),
        shot=str(wrong[0][0].png) if wrong else "",
    )


_URLISH = re.compile(r"(file://|https?://|[\w.-]+/[\w.-]+\.\w{2,4})")


def free_text(s: Shot) -> list[Word]:
    """Prose the screen is speaking in its own voice.

    Anything drawn inside a row is *data* — a document title, a file path, an
    inbox subject — and two documents in the same folder legitimately share
    most of their words. Only text outside every control is the UI explaining
    itself, and that is the text the "said it twice" and "named the wrong
    source" assertions are about.
    """
    rows = s.rows()
    field = s.text_field()
    out = []
    for w in s.words:
        if any(r.intersection(w.box) for r in rows):
            continue
        # What the user typed is not the UI's voice. Counting the query still
        # sitting in the field made "find the nvda paper" and the Brief's
        # "Goal: find the nvda paper" look like one explanation given twice.
        if field is not None and field.intersection(w.box):
            continue
        if _URLISH.search(w.text):
            continue
        out.append(w)
    return out


# Grammar, not content. Two sentences saying the same thing in two wordings
# share their nouns; what they do not share is the scaffolding around them, so
# comparing raw word sets rated "this device is offline so only the built-in
# documents are searchable" and "Offline, so only built-in documents can be
# searched" as 50% different when they say one thing.
_STOP = {
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "so", "to", "of",
    "in", "on", "for", "and", "or", "it", "its", "this", "that", "you", "your",
    "can", "will", "with", "from", "at", "as", "by", "not", "no", "only", "still",
    "has", "have", "had", "do", "does", "did", "up", "out", "we", "they",
}


def content_words(text: str) -> set[str]:
    return {w for w in normalise(text).split() if w not in _STOP and len(w) > 2}


def _sentences(s: Shot) -> list[Word]:
    return [w for w in free_text(s) if len(normalise(w.text).split()) >= 4]


def check_no_repeated_explanation(obs: Observation, ck: Checks) -> None:
    """Bug class 6: the same explanation in two wordings on one screen.

    Shipped as two near-paraphrases of the empty-result offline notice stacked
    on the same screen (standalone: "this device is offline" variants).
    """
    offenders = []
    for s in obs.shots:
        sents = _sentences(s)
        for i in range(len(sents)):
            for j in range(i + 1, len(sents)):
                a, b = content_words(sents[i].text), content_words(sents[j].text)
                if len(a) < 3 or len(b) < 3:
                    continue
                overlap = len(a & b) / len(a | b)
                if overlap >= 0.6:
                    offenders.append((s, sents[i].text, sents[j].text, overlap))
    first = offenders[0] if offenders else None
    ck.that(
        "no screen explains the same thing twice",
        not offenders,
        expected="each explanatory sentence on a screen says something new",
        seen=(
            f"{first[1]!r} and {first[2]!r} share {first[3]:.0%} of their words on {first[0].name}"
            if first
            else "none"
        ),
        shot=str(first[0].png) if first else "",
    )


_OFFLINE = re.compile(r"\b(offline|not connected|no bridge|disconnected)\b")


def check_prose_names_the_real_source(obs: Observation, ck: Checks) -> None:
    """Bug class 5: prose claims Offline above rows a Live connector just served."""
    if not obs.result_shots:
        ck.that(
            "prose about the source agrees with where the rows came from",
            False,
            expected="a result screen to assert against",
            seen="the run never produced one",
        )
        return
    s = obs.result_shots[-1]
    claims_offline = [
        w.text
        for s in obs.result_shots
        for w in free_text(s)
        if _OFFLINE.search(w.text.lower())
    ]
    # Did the bridge actually answer during this run? Its own log is the only
    # honest witness — the guest's opinion of its own connection is the thing
    # under test.
    served = [
        l
        for l in obs.bridge.splitlines()
        if l.lstrip().startswith("→ OK")
        and any(t in l for t in ("intent.resolve", "search.query", "agent.act"))
    ]
    contradiction = bool(claims_offline) and bool(served)
    ck.that(
        "prose about the source agrees with where the rows came from",
        not contradiction,
        expected="no 'offline' wording on a screen whose rows the bridge served",
        seen=(
            f"screen says {claims_offline[0]!r} but the bridge answered: {served[-1].strip()!r}"
            if contradiction
            else ("no source claim on screen" if not claims_offline else "bridge did not answer")
        ),
        shot=str(s.png),
    )


def check_keystrokes_reach_the_bridge(obs: Observation, ck: Checks) -> None:
    """Bug class 4: "i wanna work on my paper" arrived as "i wanna wy paper".

    Nothing on screen or in the kernel log revealed that. Only what the host
    received did, so that is what this reads.
    """
    if obs.received is None:
        why = "no intent.resolve / agent.act carrying a query reached the bridge at all"
        if obs.bridge_restarted:
            why += " — and the bridge was restarted under the running guest, so this is the harness's environment, not the product"
        ck.that(
            "every keystroke typed reaches the bridge unaltered",
            False,
            expected=f"the bridge to receive {obs.typed!r}",
            seen=why,
            shot=str(obs.typed_shot.png) if obs.typed_shot else "",
        )
        return
    ck.that(
        "every keystroke typed reaches the bridge unaltered",
        obs.received.strip() == obs.typed.strip(),
        expected=f"the bridge to receive {obs.typed!r}",
        seen=f"it received {obs.received!r}",
        shot=str(obs.typed_shot.png) if obs.typed_shot else "",
    )


def check_typed_text_is_echoed(obs: Observation, ck: Checks) -> None:
    """The other half of bug class 4: what the field shows you as you type."""
    s = obs.typed_shot
    if s is None:
        ck.that(
            "the query field shows exactly what was typed",
            False,
            expected="a screenshot taken after typing",
            seen="none was taken",
        )
        return
    want = normalise(obs.typed)
    hit, best = None, None
    for word in s.words:
        cand = normalise(word.text)
        # Compare at equal length only. A dropped keystroke — the bug — makes
        # the text SHORTER ("i wanna work on my paper" arrived as "i wanna wy
        # paper"); a recogniser confusing v for y does not change the length.
        # Allowing one substitution therefore tolerates the reader's eyesight
        # without tolerating the defect.
        if len(cand) != len(want):
            continue
        diff = sum(1 for a, b in zip(cand, want) if a != b)
        if best is None or diff < best[0]:
            best = (diff, word.text)
        if diff <= 1:
            hit = word.text
            break
    ck.that(
        "the query field shows exactly what was typed",
        hit is not None,
        expected=f"{obs.typed!r} on screen, character for character",
        seen=(
            f"closest line of the same length was {best[1]!r} ({best[0]} characters out)"
            if best
            else f"no line the same length; the screen reads {s.text.strip()!r}"
        ),
        shot=str(s.png),
    )


def check_field_click_stays_home(obs: Observation, ck: Checks) -> None:
    """Bug class 8: clicking the search field restarted the whole wizard."""
    ck.that(
        "clicking the search field leaves you on the home screen",
        obs.field_click_view == "home",
        expected="the derived state to still be 'home' after clicking the field",
        seen=f"it became {obs.field_click_view!r}",
        shot=str(obs.field_click_shot.png) if obs.field_click_shot else "",
    )


def check_controls_are_wired(obs: Observation, ck: Checks) -> None:
    """Bug class 1: a control drawn, clickable-looking, and doing nothing.

    Shipped twice — the recent-mail rows and the portal pill. Both were visible
    and neither routed anywhere, which no unit test can see because from inside
    a function there is nothing to route.
    """
    dead = [w for w in obs.wired if not w[1]]
    if not obs.wired:
        if not obs.swept:
            ck.note("wiring sweep NOT RUN (--no-sweep) — nothing was asserted about wiring")
            return
        ck.that(
            "every control that looks clickable does something",
            False,
            expected="at least one control swept",
            seen="the sweep found no control to click on the home screen",
        )
        return
    ck.that(
        "every control that looks clickable does something",
        not dead,
        expected=f"all {len(obs.wired)} swept control(s) to change the screen or the log",
        seen=(
            f"{len(dead)} dead: " + ", ".join(f"{d[0]} at {d[2]}" for d in dead[:3])
            if dead
            else "all responded"
        ),
        shot=dead[0][3] if dead else "",
    )


INVARIANTS = [
    check_no_fault,
    check_no_panic,
    check_home_was_reached,
    check_setup_always_offers_a_way_forward,
    check_controls_on_screen,
    check_controls_do_not_overlap,
    check_text_not_clipped,
    check_controls_are_wired,
    check_typed_text_is_echoed,
    check_keystrokes_reach_the_bridge,
    check_prose_count_matches_rows,
    check_no_duplicate_rows,
    check_no_repeated_explanation,
    check_prose_names_the_real_source,
    check_field_click_stays_home,
]


# ---------------------------------------------------------------------------
# proving the assertions can fail
# ---------------------------------------------------------------------------


def _clone_shot(s: Shot) -> Shot:
    """A shot whose words and controls can be edited without touching disk."""
    c = copy.copy(s)
    c.words = [Word(w.text, w.conf, w.box) for w in s.words]
    c._controls = list(s.controls)
    return c


def _break_fault(obs):
    obs.serial += "\nFAULT #14 PF at rip=0xffffffff80012345 cr2=0x0\n"


def _break_panic(obs):
    obs.serial += "\nPANIC: kernel/src/ui.rs:412 index out of range\n"


def _break_home(obs):
    obs.reached_home = False


def _break_setup_action(obs):
    obs.setup_steps_without_action = ["<injected> 03-setup.png"]


def _break_clipped_control(obs):
    s = _clone_shot(obs.shots[0])
    r = max(s.controls, key=lambda r: r.w * r.h)
    s._controls = [c for c in s.controls if c is not r] + [Rect(r.x, s.h - r.h // 2, r.w, r.h)]
    obs.shots = [s] + obs.shots[1:]


def _break_overlap(obs):
    s = _clone_shot(obs.shots[0])
    rs = sorted(s.controls, key=lambda r: r.w * r.h, reverse=True)[:1]
    r = rs[0]
    s._controls = list(s.controls) + [Rect(r.x + r.w // 3, r.y + r.h // 3, r.w, r.h)]
    obs.shots = [s] + obs.shots[1:]


def _break_clipped_text(obs):
    s = _clone_shot(obs.shots[0])
    w0 = s.words[0]
    s.words = [Word(w0.text, w0.conf, Rect(s.w - w0.box.w // 2, w0.box.y, w0.box.w, w0.box.h))]
    obs.shots = [s] + obs.shots[1:]


def _break_wiring(obs):
    obs.wired = [(obs.wired[0][0] if obs.wired else "tile", False, "(0,0)", "<injected>")]


def _break_echo(obs):
    if obs.typed_shot:
        s = _clone_shot(obs.typed_shot)
        s.words = [Word("something else entirely", 1.0, w.box) for w in s.words[:1]]
        obs.typed_shot = s


def _break_keystrokes(obs):
    obs.received = (obs.typed or "find the nvda paper").replace("a", "", 1)


def _break_count(obs):
    if not obs.result_shots:
        return
    s = _clone_shot(obs.result_shots[-1])
    n = len(s.rows()) + 2
    s.words = s.words + [Word(f"{n} matches for that.", 1.0, Rect(10, 10, 100, 14))]
    obs.result_shots = obs.result_shots[:-1] + [s]


def _break_duplicate_rows(obs):
    """Reproduce the arm64 Doc/Hit bug on this run's own evidence.

    The real defect pushes the same document twice, once tagged Doc and once
    tagged Hit. Both halves have to be result rows: two metadata rows sharing
    a word — "Goal: paper" above "Query: paper" — is not this bug, and seeding
    one of those let the assertion stay green on broken evidence, which is how
    a one-word goal first exposed the gap.
    """
    if not obs.result_shots:
        return
    s = _clone_shot(obs.result_shots[-1])
    rows = s.rows()
    if len(rows) < 2:
        return
    title = row_identity(" ".join(w.text for w in s.words_in(rows[0]))) or "os identity"
    # One document arriving once as Doc and once as Hit — the arm64 shape.
    for row, tag in ((rows[0], "Doc"), (rows[1], "Hit")):
        s.words = [w for w in s.words if not row.intersection(w.box)]
        s.words.append(Word(f"{tag} {title}", 1.0, Rect(row.x + 16, row.y + 8, 200, 14)))
    obs.result_shots = obs.result_shots[:-1] + [s]


def _break_repeated_explanation(obs):
    """Print one explanation twice, in two wordings, as free prose.

    Placed on the emptiest real screen and below its last row, because the
    assertion deliberately ignores text drawn inside a control — two documents
    in one folder share most of their words and are not a bug.
    """
    victim = min(obs.shots, key=lambda s: len(s.rows()))
    s = _clone_shot(victim)
    floor = max([r.bottom for r in s.rows()] or [s.nav_rule_y()]) + 8
    s.words = s.words + [
        Word("This device is offline so only the built-in documents are searchable",
             1.0, Rect(20, floor, 400, 16)),
        Word("Offline, so only built-in documents can be searched",
             1.0, Rect(20, floor + 24, 400, 16)),
    ]
    obs.shots = [s if o is victim else o for o in obs.shots]


def _break_source_prose(obs):
    if not obs.result_shots:
        return
    s = _clone_shot(obs.result_shots[-1])
    s.words = s.words + [Word("This device is offline.", 1.0, Rect(20, 260, 200, 16))]
    obs.result_shots = obs.result_shots[:-1] + [s]
    obs.bridge += "\n← CALL intent.resolve q=find the nvda paper\n→ OK intent.resolve n=3\n"


def _break_field_click(obs):
    obs.field_click_view = "setup"


PROOFS = [
    (check_no_fault, _break_fault, "a FAULT line on the serial log"),
    (check_no_panic, _break_panic, "a PANIC line on the serial log"),
    (check_home_was_reached, _break_home, "a journey that never reached home"),
    (check_setup_always_offers_a_way_forward, _break_setup_action, "a step with no primary action"),
    (check_controls_on_screen, _break_clipped_control, "a control moved past the bottom edge"),
    (check_controls_do_not_overlap, _break_overlap, "a control laid half on top of another"),
    (check_text_not_clipped, _break_clipped_text, "a line moved past the right edge"),
    (check_controls_are_wired, _break_wiring, "a control that answered nothing"),
    (check_typed_text_is_echoed, _break_echo, "a field showing different text"),
    (check_keystrokes_reach_the_bridge, _break_keystrokes, "a dropped keystroke"),
    (check_prose_count_matches_rows, _break_count, "a count two higher than the rows"),
    (check_no_duplicate_rows, _break_duplicate_rows, "one document tagged Doc and Hit"),
    (check_no_repeated_explanation, _break_repeated_explanation, "one explanation in two wordings"),
    (check_prose_names_the_real_source, _break_source_prose, "'offline' above bridge-served rows"),
    (check_field_click_stays_home, _break_field_click, "the field click landing in setup"),
]


def prove(obs: Observation) -> int:
    """Break one thing per assertion and require that assertion to go red.

    An assertion that stays green here is reported as a failure, because a test
    that cannot fail is worse than no test and this project has shipped those.
    """
    print("\nproof that each assertion can fail:")
    bad = 0
    for fn, breaker, what in PROOFS:
        damaged = copy.copy(obs)
        damaged.shots = list(obs.shots)
        damaged.result_shots = list(obs.result_shots)
        damaged.wired = list(obs.wired)
        damaged.setup_steps_without_action = list(obs.setup_steps_without_action)
        breaker(damaged)
        probe = Checks(quiet=True)
        fn(damaged, probe)
        went_red = bool(probe.failed)
        name = probe.results[-1][1] if probe.results else fn.__name__
        print(f"  {'RED ' if went_red else 'MISS'}  {name}  <- {what}")
        if not went_red:
            bad += 1
    if bad:
        print(f"\n{bad} assertion(s) stayed green on deliberately broken evidence")
    else:
        print(f"\nall {len(PROOFS)} assertions went red when their evidence was broken")
    return 1 if bad else 0


# ---------------------------------------------------------------------------
# driving the guest
# ---------------------------------------------------------------------------

KEYMAP = {
    " ": "spc",
    ".": "dot",
    ",": "comma",
    "-": "minus",
    "_": "shift-minus",
    "/": "slash",
    "?": "shift-slash",
    ":": "shift-semicolon",
}


class Qmp:
    """Minimal QMP client: enough to move a mouse and take a picture."""

    def __init__(self, path: str):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        for _ in range(160):
            try:
                self.sock.connect(path)
                break
            except (FileNotFoundError, ConnectionRefusedError):
                time.sleep(0.25)
        else:
            raise RuntimeError(f"no QMP socket at {path}")
        self.f = self.sock.makefile("rwb")
        self._read()
        self.cmd("qmp_capabilities")
        self.w = self.h = 0

    def _read(self):
        while True:
            line = self.f.readline()
            if not line:
                raise RuntimeError("QMP closed")
            msg = json.loads(line)
            if "return" in msg or "error" in msg or "QMP" in msg:
                return msg

    def cmd(self, name, **args):
        payload = {"execute": name}
        if args:
            payload["arguments"] = args
        self.f.write((json.dumps(payload) + "\n").encode())
        self.f.flush()
        reply = self._read()
        if "error" in reply:
            raise RuntimeError(f"{name}: {reply['error']}")
        return reply.get("return")

    def move(self, x, y):
        self.cmd(
            "input-send-event",
            events=[
                {"type": "abs", "data": {"axis": "x", "value": x * 32767 // self.w}},
                {"type": "abs", "data": {"axis": "y", "value": y * 32767 // self.h}},
            ],
        )

    def click(self, x, y):
        self.move(x, y)
        time.sleep(0.35)
        for down in (True, False):
            self.cmd(
                "input-send-event",
                events=[{"type": "btn", "data": {"down": down, "button": "left"}}],
            )
            time.sleep(0.12)

    def key(self, name):
        self.cmd("send-key", keys=[{"type": "qcode", "data": name}])

    def type(self, text, delay=0.12):
        """Type at a speed a person could actually reach.

        At 0.04s a key the guest dropped characters. 25 keys a second is 300
        words a minute, which says more about the harness than the kernel;
        typing at human speed keeps the test about the product. `--key-delay`
        exists so the dropped-keystroke assertion can be shown going red.
        """
        for ch in text:
            if ch in KEYMAP:
                self.key(KEYMAP[ch])
            elif ch.isalnum():
                self.key(ch.lower())
            else:
                raise ValueError(f"no key mapping for {ch!r}")
            time.sleep(delay)

    def screendump(self, path: Path):
        self.cmd("screendump", filename=str(path))


class Guest:
    """The booted ISO, plus the bookkeeping that turns it into evidence."""

    def __init__(self, qmp: Qmp, out: Path, serial: Path, titles: set[str]):
        self.qmp = qmp
        self.out = out
        self.serial = serial
        self.titles = titles
        self.shots: list[Shot] = []
        self._n = 0

    def snap(self, label: str) -> Shot:
        self._n += 1
        stem = f"{self._n:02d}-{label}"
        ppm = self.out / f"{stem}.ppm"
        png = self.out / f"{stem}.png"
        self.qmp.screendump(ppm)
        # Wait for QEMU to finish writing before reading it back.
        for _ in range(40):
            if ppm.exists() and ppm.stat().st_size > 1024:
                break
            time.sleep(0.1)
        subprocess.run(
            ["sips", "-s", "format", "png", str(ppm), "--out", str(png)],
            check=True,
            capture_output=True,
        )
        shot = Shot(stem, ppm, png)
        if not self.qmp.w:
            self.qmp.w, self.qmp.h = shot.w, shot.h
            print(f"  framebuffer: {shot.w}x{shot.h}")
        self.shots.append(shot)
        print(f"  {stem}: {shot.view(self.titles)}  ({len(shot.controls)} controls)  {png}")
        return shot

    def serial_text(self) -> str:
        return self.serial.read_text(errors="replace") if self.serial.exists() else ""


def known_titles() -> set[str]:
    """The nav titles the guest can print, scraped from the kernel.

    A hand-kept list here would have called the new Status and Brief screens
    "unknown screen" the week they landed, and the run would have failed as if
    the product were broken. Scraped, they turn up for free.
    """
    src_dir = ROOT / "kernel" / "src"
    titles = {"Home"}
    for name in ("screens.rs", "searchui.rs"):
        p = src_dir / name
        if not p.exists():
            continue
        src = p.read_text()
        for m in re.finditer(r"chrome\(\s*fb,\s*w,\s*\"([^\"]+)\"", src):
            titles.add(m.group(1))
        for m in re.finditer(r"draw_text_centered\(\s*\n?\s*w\s*/\s*2,[^;]*?\"([A-Z][A-Za-z ]{1,14})\"", src):
            titles.add(m.group(1))
    return titles


def latest_bridge_query(bridge_log: str) -> str | None:
    """What the host actually received as the user's words.

    Home's Enter goes out as `intent.resolve q=…`; a playbook goes out as
    `agent.act goal=…`. The old harness only looked for the second, so a run
    where the first arrived perfectly reported "no query reached the bridge" —
    a harness bug that read exactly like a dead COM2.
    """
    got = None
    for line in bridge_log.splitlines():
        s = line.strip().lstrip("←").strip()
        for tool, key in (("intent.resolve", "q="), ("agent.act", "goal="), ("search.query", "q=")):
            if s.startswith(f"CALL {tool}") and key in s:
                val = s.split(key, 1)[1]
                # Trailing `k=v` pairs are the bridge's, not the user's.
                val = re.split(r"\s+\w+=", val)[0]
                got = val.strip()
    return got


def reach_home(guest: Guest, obs: Observation, grants: list[str], budget: int = 24) -> Shot | None:
    """Walk whatever journey is in front of us until the home screen appears.

    No step count, no screen names, no pixel coordinates: look at the screen,
    decide, click, look again. When setup grew a step this walked one more
    time; when it lost two it stopped two earlier.
    """
    granted: set[str] = set()
    shot = guest.snap("boot")
    for _ in range(budget):
        view = shot.view(guest.titles)
        if view == "home":
            obs.reached_home = True
            return shot
        if view == "setup":
            # Grant on the way past. A journey that only ever accepts the
            # defaults never exercises a granted source, which is most of what
            # the agent does.
            #
            # Read the switch before touching it. A switch is a toggle, so
            # "click the row to grant it" un-grants anything already on — this
            # suite did exactly that on its first run and the serial log came
            # back short one capability.
            want = [g for g in grants if g.lower() not in granted]
            clicked_row = False
            for r in shot.rows():
                body = " ".join(w.text for w in shot.words_in(r)).lower()
                for g in want:
                    if g.lower() not in body:
                        continue
                    state = shot.toggle_on(r)
                    if state is None:
                        continue
                    granted.add(g.lower())
                    if state:
                        print(f"  {g!r} is already on — leaving it alone")
                        break
                    print(f"  granting {g!r}: clicking its switch at {r.centre}")
                    guest.qmp.click(*r.centre)
                    # Granting makes the guest call the host to build that
                    # source's index, and that blocks the UI loop. Clicks
                    # landing during the wait are dropped, which is how three
                    # quick toggles once registered as one.
                    time.sleep(6.0)
                    after = guest.snap("granting")
                    row_after = next(
                        (
                            q
                            for q in after.rows()
                            if abs(q.y - r.y) <= 6 and abs(q.x - r.x) <= 6
                        ),
                        None,
                    )
                    now = after.toggle_on(row_after) if row_after else None
                    obs.wired.append(
                        (f"the {g} switch", bool(now), str(r), str(after.png))
                    )
                    print(f"    switch is now {'on' if now else 'OFF — it did not take'}")
                    clicked_row = True
                    break
                if clicked_row:
                    break
            if clicked_row:
                shot = guest.snap("granted")
                continue
            pill = shot.primary_action()
            if pill is None:
                obs.setup_steps_without_action.append(str(shot.png))
                print(f"  no primary action on {shot.name} — cannot go forward")
                return None
            guest.qmp.click(*pill.centre)
            time.sleep(2.5)
        else:
            back = shot.find("Back")
            if back and back.box.y < shot.nav_rule_y():
                guest.qmp.click(*back.box.centre)
            else:
                guest.qmp.key("esc")
            time.sleep(2.0)
        shot = guest.snap("step")
    return None


def to_home(guest: Guest, shot: Shot) -> Shot | None:
    """Get back to home from wherever we are, by looking for the way back."""
    for _ in range(4):
        if shot.view(guest.titles) == "home":
            return shot
        back = shot.find("Back")
        if back and back.box.y < shot.nav_rule_y():
            guest.qmp.click(*back.box.centre)
        else:
            guest.qmp.key("esc")
        time.sleep(2.0)
        shot = guest.snap("home-again")
    return None


def open_search(guest: Guest, home: Shot) -> Shot | None:
    """Open the Search screen by clicking whichever home tile leads there.

    Which tile that is, is not assumed: every tile is tried and the one whose
    nav title comes back "Search" is the one. When the tiles are renamed or
    reordered — they have been, twice — this still finds it.
    """
    tiles = [r for r in home.controls if r.y > home.nav_rule_y() and 40 <= r.h <= 120]
    for r in tiles:
        label = " ".join(w.text for w in home.words_in(r))
        guest.qmp.click(*r.centre)
        time.sleep(2.5)
        shot = guest.snap("open-search")
        if shot.view(guest.titles) == "search":
            print(f"  the Search screen is behind the {label!r} tile")
            return shot
        back = to_home(guest, shot)
        if back is None:
            return None
        home = back
    print("  no home tile opens the Search screen")
    return None


def sweep_wiring(guest: Guest, obs: Observation, home: Shot) -> Shot:
    """Click every control home draws and require the machine to react.

    "Reacted" is either a different screen or a new line on the serial log.
    Both are observable from outside, which is the only kind of evidence that
    could have caught the recent-mail rows or the portal pill.
    """
    rule = home.nav_rule_y()
    field = home.text_field()
    targets = [
        r
        for r in home.controls
        if r.y > rule and (field is None or r != field) and r.w >= 60 and r.h >= 24
    ]
    print(f"  sweeping {len(targets)} home control(s) for wiring")
    current = home
    for i, r in enumerate(targets):
        label = " ".join(w.text for w in home.words_in(r))[:40] or f"control {i}"
        before_serial = guest.serial_text()
        before_view = current.view(guest.titles)
        guest.qmp.click(*r.centre)
        time.sleep(2.5)
        after = guest.snap(f"wired-{i}")
        after_view = after.view(guest.titles)
        grew = len(guest.serial_text()) > len(before_serial)
        changed = (after_view != before_view) or grew
        obs.wired.append((label, changed, str(r), str(after.png)))
        print(f"    {'ok  ' if changed else 'DEAD'} {label!r} -> {after_view}")
        current = after
        if after_view != "home":
            back = after.find("Back")
            if back and back.box.y < after.nav_rule_y():
                guest.qmp.click(*back.box.centre)
            else:
                guest.qmp.key("esc")
            time.sleep(2.0)
            current = guest.snap(f"wired-{i}-back")
    return current


# ---------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="/tmp/os-e2e", help="where the evidence lands")
    ap.add_argument("--iso", default=str(ROOT / "os.iso"))
    # Offline by default, because that is the only way this product runs now.
    # The default named a bridge on 127.0.0.1:7420 and the script that started
    # it went with the standalone conversion, so `make e2e` died before it
    # booted anything — and it is the only suite here that clicks.
    ap.add_argument("--bridge", default="", help="host:port of a bridge to drive against")
    ap.add_argument("--query", default="find the nvda paper")
    ap.add_argument(
        "--grant",
        default="Built-in docs,Your files",
        help="capability rows to switch on, matched against the words on the row",
    )
    ap.add_argument("--key-delay", type=float, default=0.12)
    ap.add_argument("--boot-wait", type=float, default=14.0)
    ap.add_argument("--no-prove", action="store_true", help="skip the can-it-fail proof")
    ap.add_argument("--no-sweep", action="store_true", help="skip the wiring sweep (faster)")
    args = ap.parse_args()

    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    bridge_log = ROOT / ".bridge.log"
    if args.bridge:
        # Start (or restart) the bridge before booting. Twice this project has
        # driven a guest against a bridge older than its own binary and
        # reported the old behaviour as current.
        subprocess.run(
            [str(ROOT / "scripts" / "ensure-bridge.sh")], cwd=ROOT, check=True, capture_output=True
        )
        time.sleep(1.5)
    bridge_mark = bridge_log.stat().st_size if bridge_log.exists() else 0

    qmp_path = out / "qmp.sock"
    serial = out / "serial.log"
    qemu = subprocess.Popen(
        [
            "qemu-system-x86_64",
            "-M", "q35",
            "-m", "1024",
            "-cdrom", args.iso,
            "-boot", "d",
            "-display", "none",
            "-serial", f"file:{serial}",
            # The topology make-utm.sh ships: one UHCI controller, one device.
            "-device", "piix3-usb-uhci,id=uhci0",
            "-device", "usb-tablet,bus=uhci0.0",
            "-qmp", f"unix:{qmp_path},server=on,wait=off",
            "-no-reboot",
        ]
        + (["-serial", f"tcp:{args.bridge},server=off"] if args.bridge else []),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
    )

    obs = Observation(typed=args.query, swept=not args.no_sweep)
    rc = 0
    try:
        qmp = Qmp(str(qmp_path))
        titles = known_titles()
        print(f"nav titles the kernel can print: {sorted(titles)}")
        guest = Guest(qmp, out, serial, titles)
        print("\nbooting")
        time.sleep(args.boot_wait)

        home = reach_home(guest, obs, [g for g in args.grant.split(",") if g.strip()])

        if home is not None:
            if not args.no_sweep:
                print("\nwiring sweep")
                home = sweep_wiring(guest, obs, home)
                if home.view(titles) != "home":
                    home = guest.snap("home-again")

            # Bug class 8: clicking the field must not restart the wizard.
            field = home.text_field()
            if field is not None:
                print(f"\nclicking the query field at {field.centre}")
                qmp.click(*field.centre)
                time.sleep(2.0)
                after = guest.snap("field-click")
                obs.field_click_shot = after
                obs.field_click_view = after.view(titles)
            else:
                print("\nno query field found on home")
                obs.field_click_view = "no field found on the home screen"

            print(f"\ntyping {args.query!r} at {args.key_delay}s a key")
            qmp.type(args.query, delay=args.key_delay)
            time.sleep(1.0)
            obs.typed_shot = guest.snap("typed")
            qmp.key("ret")
            time.sleep(9.0)
            answer = guest.snap("answer")
            obs.result_shots.append(answer)

            # Home Enter runs an agent Brief. The Search screen is a different
            # renderer with its own prose — the agent's sentence above the
            # rows — and that sentence is where the "5 matches above three
            # cards" and "bridge offline above bridge results" bugs lived. Go
            # and look at it too rather than assuming Brief covers it.
            home2 = to_home(guest, answer)
            if home2 is not None:
                search = open_search(guest, home2)
                if search is not None:
                    print(f"  typing {args.query!r} into the Search screen")
                    qmp.type(args.query, delay=args.key_delay)
                    time.sleep(0.8)
                    qmp.key("ret")
                    time.sleep(9.0)
                    obs.result_shots.append(guest.snap("search-answer"))

        obs.serial = guest.serial_text()
        obs.shots = guest.shots
    finally:
        qemu.terminate()
        try:
            qemu.wait(timeout=5)
        except subprocess.TimeoutExpired:
            qemu.kill()

    if bridge_log.exists():
        raw = bridge_log.read_bytes()
        # ensure-bridge.sh truncates the log when it starts a bridge, so a
        # byte offset into "the log as it was" can point past the end of "the
        # log as it is". Falling back to the whole file keeps a restart from
        # looking like a guest that never spoke.
        obs.bridge = raw[bridge_mark:].decode(errors="replace") if len(raw) >= bridge_mark else raw.decode(errors="replace")
        obs.bridge_restarted = "os-mcp-bridge starting" in obs.bridge or len(raw) < bridge_mark
    (out / "bridge.log").write_text(obs.bridge)
    obs.received = latest_bridge_query(obs.bridge)

    print("\nassertions:")
    ck = Checks()
    for fn in INVARIANTS:
        fn(obs, ck)
    rc = ck.report()

    if not args.no_prove:
        rc |= prove(obs)

    print(f"\nevidence: {len(obs.shots)} screendumps, serial log and bridge log in {out}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
