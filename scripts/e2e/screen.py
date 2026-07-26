"""Which screen is showing, and where its controls are - from the pixels.

Two questions the old harness answered by assumption:

  * "which step am I on" -> it counted how many times it had clicked Continue.
    When setup got shorter, the count ran past the end and the run ended up on
    a screen it thought was something else.
  * "where is the button" -> a literal coordinate. When the capability rows
    moved, it clicked empty space and reported that the switches did nothing.

Both are answered here by looking. The nav strip says where you are; the
controls are found by what they look like.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from . import catalog, theme
from .atlas import tracking_pct
from .frame import Frame, Rect, best_of, face_fit, ocr, score_text

# How sure a title match has to be before it is believed. Coverage overlap on a
# correct render sits above 0.9; a wrong word of the same length lands near 0.5.
TITLE_FLOOR = 0.72


@dataclass
class Screen:
    name: str
    title: str
    score: float
    nav_h: int | None
    evidence: dict = field(default_factory=dict)

    def __str__(self):
        return f"{self.name} (title={self.title!r} score={self.score:.2f})"


@dataclass
class Control:
    kind: str
    rect: Rect
    label: str = ""

    @property
    def centre(self):
        return self.rect.centre

    def __str__(self):
        return f"{self.kind}{'[' + self.label + ']' if self.label else ''}@{self.rect}"


def nav_height(f: Frame) -> int | None:
    """Where the nav rule is drawn, or None on a screen that has no nav.

    `ui.rs` draws it as a full-width 1px RULE line under the strip; setup draws
    no such line. That single row therefore separates "in the app" from "still
    in the first-boot assistant" without either being assumed.
    """
    for y in f.ramp_hairlines(theme.RULE, theme.ACCENT, min_frac=0.9):
        if 24 <= y <= 160:
            return y
    return None


def identify(f: Frame, faces) -> Screen:
    brand, title_face, btn = faces["BRAND"], faces["TITLE"], faces["BTN"]
    nav_h = nav_height(f)

    if nav_h is not None:
        nav = Rect(0, 0, f.w, nav_h)
        ink = f.text_blocks(theme.INK, nav)
        accent = f.text_blocks(theme.ACCENT, nav)
        has_back = any(
            score_text(f, b, "Back", btn, theme.ACCENT) >= TITLE_FLOOR for b in accent
        )

        home_word = catalog.home_brand()
        for b in ink:
            if b.x < f.w * 0.25:
                s = score_text(f, b, home_word, brand, theme.INK)
                if s >= TITLE_FLOOR:
                    return Screen(
                        "home", home_word, s, nav_h, {"back": has_back, "blocks": len(ink)}
                    )

        titles = catalog.app_titles()
        best = (0.0, None, None)
        for b in ink:
            if abs((b.x + b.w / 2) - f.w / 2) > f.w * 0.2:
                continue
            s, t = best_of(f, b, titles, brand, theme.INK)
            if s > best[0]:
                best = (s, t, b)
        if best[0] >= TITLE_FLOOR:
            return Screen(
                catalog.slug_for(best[1]), best[1], best[0], nav_h, {"back": has_back}
            )
        if has_back:
            # Nav rule and a Back link but no centred title: the reader draws
            # exactly that, with the document's own title below the strip.
            return Screen("reader", "", 0.0, nav_h, {"back": True})
        # Nav rule, no centred title and nowhere to go back to: that is home.
        # The wordmark above is better evidence, but it is 17px of type in the
        # top-left corner and the guest parks its own mouse cursor there - one
        # arrow over two letters and a perfectly ordinary home screen came back
        # as "unknown". The shape of the chrome does not move.
        return Screen("home", "", 0.0, nav_h, {"back": False, "by": "chrome"})

    # No nav rule: the setup assistant.
    track = tracking_pct(title_face.px, -20)
    hero_track = tracking_pct(faces["HERO"].px, -30)
    blocks = [b for b in f.text_blocks(theme.INK) if b.h >= 15]
    faces_for = catalog.setup_face()
    best = (0.0, None, None)
    for slug, heading in catalog.setup_headings():
        fc = faces["HERO"] if faces_for.get(heading) == "HERO" else title_face
        tr = hero_track if fc is faces["HERO"] else track
        for b in blocks:
            s = score_text(f, b, heading, fc, theme.INK, tracking64=tr)
            if s > best[0]:
                best = (s, slug, heading)
    if best[0] >= TITLE_FLOOR:
        return Screen(f"setup:{best[1]}", best[2], best[0], None, {"blocks": len(blocks)})
    return Screen("unknown", "", best[0], None, {"blocks": len(blocks)})


# --- controls -----------------------------------------------------------


def primary(f: Frame, faces) -> Control | None:
    """The filled accent pill: Continue, Start, or whatever it is called now.

    Found by shape, so renaming the label or moving the footer changes nothing
    here. Requiring both width and height is what keeps a selected row - drawn
    as a 520px accent outline, three times the pill's width - from winning.
    """
    blobs = [b for b in f.solid_blobs(theme.ACCENT, min_w=100, min_h=24) if b.h <= 90]
    if not blobs:
        return None
    r = max(blobs, key=lambda b: b.w * b.h)
    label = ""
    inner = f.text_blocks(theme.BG, r.inset(4), bg=theme.ACCENT)
    if inner:
        blk = max(inner, key=lambda b: b.w)
        label = ocr(f, blk, faces["BTN"], theme.BG, bg=theme.ACCENT)
    return Control("primary", r, label.strip())


def back(f: Frame, faces) -> Control | None:
    """The nav Back affordance, wherever the strip happens to be."""
    nav_h = nav_height(f) or 70
    for b in f.text_blocks(theme.ACCENT, Rect(0, 0, f.w, nav_h)):
        if score_text(f, b, "Back", faces["BTN"], theme.ACCENT) >= TITLE_FLOOR:
            # Widen to the hit zone the kernel registers around the glyphs.
            return Control("back", Rect(b.x - 12, b.y - 10, b.w + 24, b.h + 20), "Back")
    return None


def go_back(f: Frame, faces) -> Control | None:
    """Setup's centred "Go Back" link. It has been drawn off the bottom edge."""
    for b in f.text_blocks(theme.ACCENT):
        if b.y < (nav_height(f) or 70):
            continue
        if score_text(f, b, "Go Back", faces["BTN"], theme.ACCENT) >= TITLE_FLOOR:
            return Control("go_back", Rect(b.x - 12, b.y - 8, b.w + 24, b.h + 20), "Go Back")
    return None


def field(f: Frame, faces) -> Control | None:
    """The text input: a wide RULE-outlined rounded box in the upper half.

    Home and Search both draw one; the placeholder inside is read back so a
    caller can tell which copy is showing without knowing what it says.
    """
    boxes = [
        b
        for b in f.boxes(theme.RULE, min_w=240, min_h=30)
        if b.h <= 90 and b.y < f.h * 0.6
    ]
    if not boxes:
        return None
    r = min(boxes, key=lambda b: b.y)
    muted = _no_caret(f, f.text_blocks(theme.MUTED, r.inset(6)), theme.MUTED)
    ink = _no_caret(f, f.text_blocks(theme.INK, r.inset(6), min_alpha=INK_LEVEL))
    # Full-strength ink means someone has typed; only an empty field shows the
    # muted placeholder. Preferring muted got this backwards: the anti-aliased
    # fringe of black text is mid-grey, which is indistinguishable from muted
    # type at partial coverage, so a field reading "nvda" came back as "r".
    inner, fg = (ink, theme.INK) if ink else (muted, theme.MUTED)
    label = ""
    if inner:
        x0 = min(b.x for b in inner)
        y0 = min(b.y for b in inner)
        blk = Rect(
            x0,
            y0,
            max(b.right for b in inner) - x0,
            max(b.bottom for b in inner) - y0,
        )
        label = read_line(f, blk, faces, fg)
    return Control("field", r, label.strip())


def _no_caret(f: Frame, blocks, fg=theme.INK):
    """Trim the insertion caret out of a run of text blocks.

    It is a solid 2px bar as tall as the field, drawn in INK like the text, and
    it sits close enough to the last glyph to land inside the same block. Left
    in, it stretches the line's box to 24px and adds a column of coverage no
    string can explain - "nvda" read back as "r", and a perfectly good field
    was reported as a dropped-keystroke bug.

    A caret is a *narrow run of near-block-height columns*. Both halves matter:
    thresholding on height alone against the median column deleted the letters
    too, because anti-aliased type only carries a pixel or two of full-strength
    ink per column and the median sits down there with them.
    """
    a = f.alpha(fg)
    out = []
    for b in blocks:
        sub = a[b.y : b.bottom, b.x : b.right] >= INK_LEVEL
        heights = sub.sum(axis=0)
        if not heights.any():
            continue
        if b.w <= 3 and b.h >= 14:
            # Nothing but the caret: an empty field still blinks one, and kept
            # it read the field back as "|" and outranked the placeholder.
            continue
        # Otherwise a caret is much taller than the type around it, and taller
        # than any glyph in absolute terms. Scaling the threshold off the block
        # height alone flagged the ascender of a "d" as a caret on the frames
        # where the caret happened to be blinked off, and deleted a real letter.
        lit = heights[heights > 0]
        tall = heights >= max(14, int(np.percentile(lit, 90) * 1.5))
        drop = np.zeros(len(heights), dtype=bool)
        run = 0
        for i, t in enumerate(list(tall) + [False]):
            if t:
                run += 1
                continue
            if 0 < run <= 3:
                drop[i - run : i] = True
            run = 0
        keep = np.nonzero((heights > 0) & ~drop)[0]
        if len(keep) == 0:
            # The whole block was the caret. Dropping it is the point: an empty
            # search field still blinks one, and keeping it made the field read
            # back as "|" and outrank the placeholder underneath.
            continue
        # Vertical extent from the full coverage map, not the thresholded one:
        # anti-aliased ascenders live below INK_LEVEL, and measuring the line's
        # height off the threshold made it 8px tall in a 17px face - which
        # throws off both the baseline search and the choice of cut.
        soft = a[b.y : b.bottom, b.x : b.right] >= 45
        rows_ = np.nonzero(soft[:, keep].any(axis=1))[0]
        out.append(
            Rect(
                b.x + int(keep.min()),
                b.y + int(rows_[0]),
                int(keep.max() - keep.min()) + 1,
                int(rows_[-1] - rows_[0]) + 1,
            )
        )
    return out


def rows(f: Frame, below: int | None = None) -> list[Rect]:
    """List rows / cards, by their drawn border.

    Three palettes, not two. An unselected row is CARD_BORDER and a chosen one
    is ACCENT, but a row that is merely *on* - a granted capability - is drawn
    with TINT_BORDER (ui.rs: `if on { theme::TINT_BORDER }`).

    Missing that third one made the suite report "6 rows but 7 switches - a row
    you cannot grant is a row that does nothing" against a Capabilities screen
    that plainly had seven of each. The invisible row was "Save skills", the
    one that had just been switched on. A detector that cannot see a control in
    one of its states will eventually hide a real missing control, so it is
    worth naming all three rather than widening a tolerance.
    """
    out = f.boxes(theme.CARD_BORDER, min_w=200, min_h=28)
    for palette in (theme.ACCENT, theme.TINT_BORDER):
        if palette is None:
            continue
        out += [
            b
            for b in f.boxes(palette, min_w=200, min_h=28)
            if not any(b.overlaps(o) for o in out)
        ]
    if below is not None:
        out = [b for b in out if b.y >= below]
    out.sort(key=lambda b: (b.y, b.x))
    return out


def content_rows(f: Frame, faces) -> list[Rect]:
    """Rows below the input field - i.e. results, not the field itself."""
    fld = field(f, faces)
    cut = fld.rect.bottom if fld else (nav_height(f) or 56)
    return rows(f, below=cut + 4)


# Coverage above this is full-strength INK; MUTED text over white tops out
# around 136 when measured against the INK ramp, because the two greys are
# nearly collinear with the page. Thresholding on level is what separates a
# row's title from the blurb printed beside it in the same pixel row.
INK_LEVEL = 170


def ink_lines(f: Frame, region: Rect) -> list[Rect]:
    """Full-strength ink text in `region`, one rect per rendered line."""
    blocks = f.text_blocks(theme.INK, region, min_alpha=INK_LEVEL)
    lines: list[Rect] = []
    for b in sorted(blocks, key=lambda r: (r.y, r.x)):
        for i, ln in enumerate(lines):
            if b.y < ln.bottom + 3 and ln.y < b.bottom + 3:
                x0, y0 = min(ln.x, b.x), min(ln.y, b.y)
                lines[i] = Rect(
                    x0, y0, max(ln.right, b.right) - x0, max(ln.bottom, b.bottom) - y0
                )
                break
        else:
            lines.append(b)
    return lines


def read_line(f: Frame, block: Rect, faces, fg=theme.INK, bg=None) -> str:
    """Read one line of text without being told which cut it was set in.

    The cut is chosen by how well any single glyph of it lands on the start of
    the line - a measurement, not the height heuristic that read "Email" as
    "B}ndtI" because a word with no descender is short. Only faces that fit
    about as well get their reads compared, and then the better-explaining read
    wins; ranking every face's read by coverage alone lets a wrong cut win by
    covering a stray mark the right one honestly leaves unexplained.
    """
    scored = []
    for fc in faces.values():
        if fc.px * 0.35 <= block.h <= fc.px * 1.15:
            scored.append((face_fit(f, block, fc, fg, bg), fc))
    if not scored:
        scored = [(0.0, faces["BODY"])]
    scored.sort(key=lambda p: -p[0])
    top = scored[0][0]
    best, best_score = "", -1.0
    for fit, fc in scored[:3]:
        if fit < top - 0.06:
            break
        txt = ocr(f, block, fc, fg, bg)
        if not txt:
            continue
        sc = score_text(f, block, txt, fc, fg, bg, slack=2, pad=1)
        if sc > best_score:
            best, best_score = txt, sc
    return best.strip()


def row_titles(f: Frame, faces, rs=None) -> list[str]:
    """The first line of full-strength ink inside each row.

    On a Brief row the kind - "Hit", "Doc", "Plan" - is drawn in MUTED above
    the thing itself, which is drawn in INK. Reading the ink line is therefore
    reading the document, not the label the agent filed it under, and that is
    exactly what has to be compared: the arm64 guest listed "Doc: os identity"
    and "Hit: os identity" in one report - one document, twice, under two
    labels. Comparing the labelled strings calls those two different rows.
    """
    out = []
    for r in rs if rs is not None else content_rows(f, faces):
        lines = ink_lines(f, Rect(r.x + 4, r.y + 2, r.w - 8, r.h - 4))
        out.append(read_line(f, lines[0], faces) if lines else "")
    return out


def row_kinds(f: Frame, faces, rs=None) -> list[tuple[str, bool]]:
    """The label a row is filed under, and whether the row is openable.

    `draw_brief` draws that label in ACCENT when the row can be opened and in
    MUTED when it is only commentary. So the screen itself says which rows are
    results - no list of magic words needed, and a new label the other session
    invents next week is classified correctly the moment it appears.
    """
    out = []
    for r in rs if rs is not None else content_rows(f, faces):
        reg = Rect(r.x + 4, r.y + 2, r.w - 8, r.h - 4)
        for colour, openable in ((theme.ACCENT, True), (theme.MUTED, False)):
            blocks = f.text_blocks(colour, reg)
            if blocks:
                top = min(blocks, key=lambda b: b.y)
                out.append((read_line(f, top, faces, colour), openable))
                break
        else:
            out.append(("", False))
    return out


def tiles(f: Frame, faces) -> list[Control]:
    """The home destinations, labelled by the H2 title drawn inside each.

    A tile is one of a row of same-height cards side by side, which is what
    tells them apart from a stacked list. Taking "any card wider than 120px"
    picked up the seven capability rows as tiles and then reported them
    overlapping each other, because a stacked list read through a
    tile-shaped filter always looks like a pile.
    """
    if nav_height(f) is None:
        return []  # setup draws no nav; it has rows, not tiles
    fld = field(f, faces)
    cut = fld.rect.bottom if fld else (nav_height(f) or 56)
    cards = [
        r
        for r in f.boxes(theme.CARD_BORDER, min_w=100, min_h=40)
        if r.y >= cut and r.w <= f.w * 0.4
    ]
    bands: dict[int, list[Rect]] = {}
    for r in cards:
        bands.setdefault(r.y // 6, []).append(r)
    out = []
    for band in bands.values():
        if len(band) < 2:
            continue
        for r in band:
            blocks = f.text_blocks(theme.INK, Rect(r.x + 6, r.y + 6, r.w - 12, r.h - 12))
            label = ocr(f, blocks[0], faces["H2"], theme.INK).strip() if blocks else ""
            out.append(Control("tile", r, label))
    out.sort(key=lambda c: (c.rect.y, c.rect.x))
    return out


def status_dot(f: Frame) -> Control | None:
    """The nav status dot, whichever way it is currently tinted."""
    nav_h = nav_height(f) or 56
    for name, col in (("online", theme.ONLINE), ("offline", theme.OFFLINE)):
        if col is None:
            continue
        m = f.mask(col, tol=30)[:nav_h]
        ys, xs = m.nonzero()
        if len(xs) >= 20:
            return Control(
                "status_dot",
                Rect(
                    int(xs.min()),
                    int(ys.min()),
                    int(np.ptp(xs)) + 1,
                    int(np.ptp(ys)) + 1,
                ),
                name,
            )
    return None


def toggles(f: Frame) -> list[Control]:
    """Capability switches, on (accent track) or off (rule track).

    The tolerance matters: RULE and CARD_BORDER are 22 apart, so the loose
    match used for finding a pill swept every row border into the mask, every
    row's span became the full card width, and a screen with seven switches
    reported one.
    """
    out = []
    for state, col in (("on", theme.ACCENT), ("off", theme.RULE)):
        for b in f.solid_blobs(col, min_w=24, min_h=14, tol=10, density=0.5):
            if 24 <= b.w <= 60 and 14 <= b.h <= 32:
                out.append(Control("toggle", b, state))
    out.sort(key=lambda c: c.rect.y)
    return out


FINDERS = {
    "primary": primary,
    "back": back,
    "go_back": go_back,
    "field": field,
    "status_dot": lambda f, faces: status_dot(f),
}


def find_control(f: Frame, kind: str, faces):
    """Locate a control by appearance. `None` when it is not on this screen."""
    if kind in FINDERS:
        return FINDERS[kind](f, faces)
    if kind == "rows":
        return content_rows(f, faces)
    if kind == "tiles":
        return tiles(f, faces)
    if kind == "toggles":
        return toggles(f)
    raise ValueError(f"unknown control kind {kind!r}")


def controls(f: Frame, faces) -> list[tuple[str, Rect]]:
    """Every control this frame draws, as (kind, rect)."""
    out = []
    for kind in ("primary", "back", "go_back", "field", "status_dot"):
        c = find_control(f, kind, faces)
        if c:
            out.append((kind, c.rect))
    out += [("tile", c.rect) for c in tiles(f, faces)]
    out += [("row", r) for r in content_rows(f, faces)]
    return out


def clashes(f: Frame, faces) -> list[str]:
    """Controls drawn on top of each other, ignoring one control found twice.

    A home tile is both a "tile" and a "row"; two finders returning the same
    rectangle is not a layout bug. Anything else sharing pixels is - the setup
    footer once landed on the last capability row and the only way out of the
    step was forward.
    """
    found = controls(f, faces)
    out = []
    for i, (ka, a) in enumerate(found):
        for kb, b in found[i + 1 :]:
            if not a.overlaps(b):
                continue
            ix = max(0, min(a.right, b.right) - max(a.x, b.x))
            iy = max(0, min(a.bottom, b.bottom) - max(a.y, b.y))
            if ix * iy > 0.9 * min(a.w * a.h, b.w * b.h):
                continue  # the same control, found twice
            out.append(f"{ka}{a} over {kb}{b}")
    return out


def offscreen(f: Frame, faces) -> list[str]:
    """Controls that do not fit on the framebuffer.

    "Go Back" was once drawn below the bottom edge. It was in the layout, it
    was in the hit table, and nobody could see or reach it.
    """
    # Flush against an edge means clipped. Screendumps cannot show what fell
    # off, so a control whose rectangle stops exactly at the framebuffer
    # boundary is the only trace left of one that was drawn past it.
    return [
        f"{k}{r}"
        for k, r in controls(f, faces)
        if r.x <= 0 or r.y <= 0 or r.right >= f.w or r.bottom >= f.h
    ]
