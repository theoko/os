#!/usr/bin/env python3
"""Prove the screen checks can fail.

A test that cannot go red is worse than no test, and this project has shipped
those. Every assertion in `driver.py` that is decided by looking at the
framebuffer is exercised here twice: once against a real captured frame, where
it must pass, and once against the same frame with the corresponding defect
painted into it, where it must fail.

The injuries are the real bug classes, drawn into real pixels:

  1. a control drawn but not wired      -> the tile is there, the click is a
                                           no-op (simulated by handing the
                                           check an unchanged screen)
  2. a control off-screen / overlapping -> shift the primary pill under the
                                           bottom edge; slide a tile onto its
                                           neighbour
  3. prose counting rows that are not   -> delete a drawn result row
     drawn
  4. dropped keystrokes                 -> erase a glyph from the query field
  7. the same document listed twice     -> copy one result row's text over
                                           another's
  8. a click that restarts setup        -> hand the check a setup screen

`kernel/src/` belongs to another session, so the product itself is not edited
to make these go red. Painting the defect into the framebuffer tests the same
thing the run tests - these checks only ever see pixels - and it can be run
without a VM.

    ./scripts/e2e/selftest.py --frames /tmp/e2e-run
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))
    __package__ = "scripts.e2e"

from . import atlas, screen, theme  # noqa: E402
from .driver import duplicate_results, frame_score  # noqa: E402
from .frame import Frame, Rect  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent.parent


def copy(f: Frame) -> Frame:
    return Frame(f.px.copy(), f.path)


def paint(f: Frame, r: Rect, colour=theme.BG) -> None:
    f.px[max(0, r.y) : r.bottom, max(0, r.x) : r.right] = np.array(colour, dtype=np.uint8)
    f._alpha.clear()
    f._masks.clear()


def move(f: Frame, r: Rect, dx: int, dy: int) -> None:
    """Cut a rectangle out and paste it somewhere else, background behind."""
    patch = f.px[r.y : r.bottom, r.x : r.right].copy()
    paint(f, r)
    y, x = r.y + dy, r.x + dx
    h = min(patch.shape[0], f.h - y)
    w = min(patch.shape[1], f.w - x)
    if h > 0 and w > 0 and y >= 0 and x >= 0:
        f.px[y : y + h, x : x + w] = patch[:h, :w]
    f._alpha.clear()
    f._masks.clear()


class Report:
    def __init__(self):
        self.rows = []

    def case(self, name, healthy_ok, injured_ok):
        """`healthy_ok` must be True, `injured_ok` must be False."""
        good = bool(healthy_ok) and not injured_ok
        self.rows.append((good, name, healthy_ok, injured_ok))
        state = "OK  " if good else "BAD "
        print(
            f"  {state} {name:52s} healthy={'pass' if healthy_ok else 'FAIL'} "
            f"injured={'pass' if injured_ok else 'FAIL'}"
        )
        return good

    def report(self):
        bad = [r for r in self.rows if not r[0]]
        print(f"\n{len(self.rows) - len(bad)}/{len(self.rows)} checks are falsifiable")
        for _, name, h, i in bad:
            why = (
                "does not hold on a healthy screen"
                if not h
                else "still passes on a broken screen - this assertion cannot fail"
            )
            print(f"  BROKEN: {name} - {why}")
        return 1 if bad else 0


def draw_card(f: Frame, r: Rect, colour=theme.CARD_BORDER) -> None:
    """Draw a card outline, the way `fill_round_rect` leaves one: a 1px ring.

    Injuring an overlap by *moving* a control does not work - paint is
    destructive, and a pill dropped on a row erases the row's edges, so the row
    stops being detected and the two never overlap. Adding a card over two
    existing ones leaves both of them intact and genuinely on top.
    """
    c = np.array(colour, dtype=np.uint8)
    f.px[r.y, r.x : r.right] = c
    f.px[r.bottom - 1, r.x : r.right] = c
    f.px[r.y : r.bottom, r.x] = c
    f.px[r.y : r.bottom, r.right - 1] = c
    f._alpha.clear()
    f._masks.clear()


def pick(frames: Path, want: str, faces, richest=False):
    """A captured frame that identifies as `want`.

    `richest` picks the one with the most rows on it - a Brief that came back
    empty has nothing to duplicate or miscount, so injecting a defect into it
    proves nothing either way.
    """
    hits = [f for p in sorted(frames.glob("*.ppm")) for f in [Frame.load(p)]
            if screen.identify(f, faces).name == want]
    if not hits:
        raise SystemExit(
            f"no frame in {frames} shows {want!r} - run driver.py --keep-ppm first"
        )
    if richest:
        return max(hits, key=lambda f: len(screen.content_rows(f, faces)))
    return hits[0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--frames", default="/tmp/os-ui", help="a driver.py --out directory")
    args = ap.parse_args()
    frames = Path(args.frames)
    faces = atlas.load()
    r = Report()

    home = pick(frames, "home", faces)
    brief = pick(frames, "brief", faces, richest=True)
    caps = pick(frames, "setup:caps", faces)
    print(f"frames: home={home.path.name} brief={brief.path.name} caps={caps.path.name}\n")

    # --- which screen am I on ------------------------------------------
    print("screen identification:")
    r.case(
        "identify(home) is not just 'any screen with a nav'",
        screen.identify(home, faces).name == "home",
        screen.identify(brief, faces).name == "home",
    )
    r.case(
        "identify(brief) reads the nav title, not the layout",
        screen.identify(brief, faces).name == "brief",
        screen.identify(pick(frames, "status", faces), faces).name == "brief",
    )
    blanked = copy(brief)
    paint(blanked, Rect(0, 0, brief.w, 56))
    r.case(
        "a screen with its nav strip erased is not still 'brief'",
        screen.identify(brief, faces).name == "brief",
        screen.identify(blanked, faces).name == "brief",
    )

    # --- bug class 8: clicking the field restarts setup -----------------
    print("\nclicking the field must not restart setup:")
    r.case(
        "after clicking the field the guest is still on home",
        screen.identify(home, faces).name == "home",
        screen.identify(caps, faces).name == "home",
    )

    # --- bug class 2: off-screen and overlapping controls ---------------
    print("\nlayout:")
    onscreen = Rect(0, 0, caps.w, caps.h)

    def all_inside(f):
        return not screen.offscreen(f, faces)

    sunk = copy(caps)
    pill = screen.primary(caps, faces).rect
    # Far enough down that the pill is clipped by the bottom edge, but not so
    # far that it vanishes: a control that is simply absent is caught by the
    # "this step is a dead end" check instead, and proving that one here would
    # prove nothing about this one.
    move(sunk, pill, 0, caps.h - pill.y - 30)
    r.case(
        "the primary action is inside the framebuffer",
        all_inside(caps),
        all_inside(sunk),
    )

    # The shipped version of this was the setup footer laid over the last
    # capability row, so the final switch could not be hit. Reproduce the shape
    # of it: a card drawn across two of the home destinations.
    piled = copy(home)
    t0 = screen.tiles(home, faces)[0].rect
    draw_card(piled, Rect(t0.x + 170, t0.y + 30, 300, 60))
    r.case(
        "nothing is drawn on top of anything else",
        not screen.clashes(home, faces),
        not screen.clashes(piled, faces),
    )

    # --- bug class 1: drawn but not wired -------------------------------
    print("\nwiring:")

    def went_somewhere(before, after):
        return screen.identify(after, faces).name != screen.identify(before, faces).name

    r.case(
        "clicking a destination changes the screen",
        went_somewhere(home, brief),
        went_somewhere(home, copy(home)),
    )

    # --- bug class 4: dropped keystrokes --------------------------------
    print("\nkeystrokes:")
    typed = None
    for p in sorted(frames.glob("*.ppm")):
        f = Frame.load(p)
        fld = screen.field(f, faces)
        if fld and fld.label and screen.identify(f, faces).name == "home":
            ink = screen._no_caret(
                f, f.text_blocks(theme.INK, fld.rect.inset(6), min_alpha=screen.INK_LEVEL)
            )
            if ink:
                typed = (f, fld, fld.label)
                break
    if typed is None:
        print("  SKIP  no frame with text typed into the field")
    else:
        f, fld, text = typed
        chopped = copy(f)
        ink = screen._no_caret(
            f, f.text_blocks(theme.INK, fld.rect.inset(6), min_alpha=screen.INK_LEVEL)
        )
        blk = ink[0]
        # Rub out the last glyph: exactly what a dropped keystroke looks like.
        paint(chopped, Rect(blk.right - max(6, blk.w // len(text)), blk.y - 2, 12, blk.h + 4))
        r.case(
            f"the field shows every character of {text!r}",
            frame_score(f, fld, text, faces) >= 0.95,
            frame_score(chopped, screen.field(chopped, faces), text, faces) >= 0.95,
        )

    # --- bug class 3: prose counting rows that are not drawn ------------
    print("\nrow counts:")
    rows = screen.content_rows(brief, faces)
    claimed = len(rows)
    thinned = copy(brief)
    paint(thinned, Rect(rows[-1].x - 2, rows[-1].y - 2, rows[-1].w + 4, rows[-1].h + 4))
    r.case(
        f"the screen draws the {claimed} rows it claims",
        len(screen.content_rows(brief, faces)) == claimed,
        len(screen.content_rows(thinned, faces)) == claimed,
    )

    # --- bug class 7: the same result listed twice ----------------------
    print("\nduplicate results:")
    titles = screen.row_titles(brief, faces, rows)
    kinds = screen.row_kinds(brief, faces, rows)
    # Prefer two openable rows - that is the shipped bug exactly. Failing that,
    # any two rows filed under the same label will do; both are duplicates the
    # check is supposed to catch.
    openable = [i for i, (_, o) in enumerate(kinds) if o]
    pair = None
    if len(openable) >= 2:
        pair = (openable[0], openable[1])
    else:
        seen = {}
        for i, (k, _) in enumerate(kinds):
            if k and k in seen:
                pair = (seen[k], i)
                break
            seen[k] = i
    if pair is None:
        print("  SKIP  this Brief has no two rows that could be duplicates")
    else:
        a, b = pair
        doubled = copy(brief)
        src, dst = rows[a], rows[b]
        h = min(src.h, dst.h) - 4
        w = min(src.w, dst.w) - 4
        # Copy one result's body over another's, leaving both labels alone:
        # "Doc: os identity" twice, exactly the arm64 report.
        doubled.px[dst.y + 2 : dst.y + 2 + h, dst.x + 2 : dst.x + 2 + w] = brief.px[
            src.y + 2 : src.y + 2 + h, src.x + 2 : src.x + 2 + w
        ]
        doubled._alpha.clear()
        doubled._masks.clear()
        drows = screen.content_rows(doubled, faces)
        r.case(
            "no result is listed twice",
            not duplicate_results(titles, kinds),
            not duplicate_results(
                screen.row_titles(doubled, faces, drows),
                screen.row_kinds(doubled, faces, drows),
            ),
        )

    # --- capability switches --------------------------------------------
    print("\ncapability switches:")

    def switch_per_row(f):
        return len(screen.toggles(f)) == len(screen.rows(f))

    lost = copy(caps)
    sw = screen.toggles(caps)
    paint(lost, Rect(sw[-1].rect.x - 2, sw[-1].rect.y - 2, sw[-1].rect.w + 4, sw[-1].rect.h + 4))
    r.case(
        "every capability row draws a switch",
        switch_per_row(caps),
        switch_per_row(lost),
    )

    off = copy(caps)
    on_idx = next((i for i, c in enumerate(sw) if c.label == "on"), None)
    if on_idx is None:
        print("  SKIP  no switch is on in this frame")
    else:
        # Repaint the on switch in the off track colour: a grant that says it
        # took and did not.
        t = sw[on_idx].rect
        paint(off, t, theme.RULE)
        r.case(
            "a granted switch reads back on",
            screen.toggles(caps)[on_idx].label == "on",
            screen.toggles(off)[on_idx].label == "on",
        )

    return r.report()


if __name__ == "__main__":
    sys.exit(main())
