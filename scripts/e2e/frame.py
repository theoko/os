"""A screendump, and the primitives for reading things off it.

Everything above this file works in terms of *appearance*: a pill of accent
colour, a hairline of rule colour, a run of glyph coverage that spells "Brief".
Nothing here knows where any control is supposed to be, because the whole
reason the previous harness kept failing is that it did.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from . import theme
from .atlas import Face


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    w: int
    h: int

    @property
    def centre(self) -> tuple[int, int]:
        return (self.x + self.w // 2, self.y + self.h // 2)

    @property
    def right(self) -> int:
        return self.x + self.w

    @property
    def bottom(self) -> int:
        return self.y + self.h

    def contains(self, px: int, py: int) -> bool:
        return self.x <= px < self.right and self.y <= py < self.bottom

    def overlaps(self, o: "Rect") -> bool:
        return not (
            self.right <= o.x or o.right <= self.x or self.bottom <= o.y or o.bottom <= self.y
        )

    def inset(self, d: int) -> "Rect":
        return Rect(self.x + d, self.y + d, max(0, self.w - 2 * d), max(0, self.h - 2 * d))

    def __repr__(self):
        return f"Rect({self.x},{self.y},{self.w}x{self.h})"


def read_ppm(path: Path) -> np.ndarray:
    """QEMU writes binary P6: ASCII header, then raw RGB triples."""
    data = path.read_bytes()
    if data[:2] != b"P6":
        raise RuntimeError(f"{path} is not a binary PPM")
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
    pos += 1
    w, h, _ = fields
    px = np.frombuffer(data, dtype=np.uint8, count=w * h * 3, offset=pos)
    return px.reshape(h, w, 3).copy()


def write_ppm(path: Path, px: np.ndarray) -> None:
    h, w, _ = px.shape
    path.write_bytes(b"P6\n%d %d\n255\n" % (w, h) + px.astype(np.uint8).tobytes())


def to_png(ppm: Path, png: Path) -> None:
    subprocess.run(
        ["sips", "-s", "format", "png", str(ppm), "--out", str(png)],
        check=True,
        capture_output=True,
    )


class Frame:
    """One screendump plus cached derivations."""

    def __init__(self, px: np.ndarray, path: Path | None = None):
        self.px = px
        self.h, self.w = px.shape[:2]
        self.path = path
        self._alpha: dict[tuple, np.ndarray] = {}
        self._masks: dict[tuple, np.ndarray] = {}

    @classmethod
    def load(cls, path: Path) -> "Frame":
        return cls(read_ppm(Path(path)), Path(path))

    # --- colour ---------------------------------------------------------

    def mask(self, colour, tol: int = 16) -> np.ndarray:
        """Pixels within `tol` of `colour` on every channel."""
        key = (tuple(colour), tol)
        if key not in self._masks:
            c = np.array(colour, dtype=np.int16)
            self._masks[key] = (np.abs(self.px.astype(np.int16) - c) <= tol).all(axis=2)
        return self._masks[key]

    def alpha(self, fg, bg=None, agree: int = 18) -> np.ndarray:
        """Recover the coverage the kernel blended, for text of colour `fg`.

        `Surface::blend` composites `fg` over `bg` at alpha `a`, so `a` is
        recoverable per channel. Channels where `fg` and `bg` are close carry
        no information and are dropped; if the surviving channels disagree the
        pixel is not this colour at all and the coverage is zero.

        That last clause is what keeps the green status dot out of a search for
        black text: a coloured blob yields wildly different per-channel
        estimates, so it reads as background instead of as a letter.
        """
        bg = tuple(theme.BG if bg is None else bg)
        key = (tuple(fg), bg, agree)
        if key in self._alpha:
            return self._alpha[key]

        f = np.array(fg, dtype=np.float32)
        b = np.array(bg, dtype=np.float32)
        d = b - f
        use = np.abs(d) >= 40
        if not use.any():
            raise ValueError(f"fg {fg} is indistinguishable from bg {bg}")
        p = self.px.astype(np.float32)
        est = np.zeros_like(p)
        est[:, :, use] = (b[use] - p[:, :, use]) * 255.0 / d[use]
        sel = est[:, :, use]
        lo, hi = sel.min(axis=2), sel.max(axis=2)
        a = sel.mean(axis=2)
        # Agreement scales with coverage. A flat tolerance let the anti-aliased
        # fringe of grey text pass as faint accent - every MUTED label on the
        # Brief screen registered as an accent one, and every row read as
        # openable.
        ok = (hi - lo <= np.maximum(agree, a * 0.12)) & (a > 6) & (a < 300)
        out = np.where(ok, np.clip(a, 0, 255), 0).astype(np.float32)
        self._alpha[key] = out
        return out

    def is_blank(self, r: Rect, tol: int = 6) -> bool:
        """True if every pixel in `r` is the page background.

        A control drawn off-screen leaves its slot blank; a control drawn on
        top of another leaves the slot below it blank. Both have shipped.
        """
        sub = self.px[max(0, r.y) : r.bottom, max(0, r.x) : r.right]
        if sub.size == 0:
            return True
        c = np.array(theme.BG, dtype=np.int16)
        return bool((np.abs(sub.astype(np.int16) - c) <= tol).all())

    def ink_fraction(self, r: Rect, tol: int = 6) -> float:
        sub = self.px[max(0, r.y) : r.bottom, max(0, r.x) : r.right]
        if sub.size == 0:
            return 0.0
        c = np.array(theme.BG, dtype=np.int16)
        same = (np.abs(sub.astype(np.int16) - c) <= tol).all(axis=2)
        return float(1.0 - same.mean())

    # --- structure ------------------------------------------------------

    def ramp_hairlines(self, c0, c1, min_frac=0.9, tol=14, t_max=0.7) -> list[int]:
        """Full-width 1px rules whose colour lies anywhere on the `c0`->`c1` ramp.

        `ui::paint_chill_rule` lerps the nav separator between `theme::RULE`
        and `theme::ACCENT` on a 60Hz breath, so at the instant a screendump is
        taken it can be any colour in between - it was (142,179,218) in the
        frame that made this necessary. Matching the flat RULE colour found the
        rule on some frames and not others, and the home screen came back as
        "unknown" depending on when the shutter fell.
        """
        a = np.array(c0, dtype=np.float32)
        b = np.array(c1, dtype=np.float32)
        d = b - a
        lead = int(np.argmax(np.abs(d)))
        px = self.px.astype(np.float32)
        t = (px[:, :, lead] - a[lead]) / d[lead]
        pred = a[None, None, :] + t[:, :, None] * d[None, None, :]
        on = (np.abs(px - pred) <= tol).all(axis=2) & (t >= -0.05) & (t <= t_max)
        return [int(y) for y in np.nonzero(on.mean(axis=1) >= min_frac)[0]]

    def hairlines(self, colour, min_frac: float = 0.9) -> list[int]:
        """Rows that are a near-full-width 1px rule of `colour`.

        The nav separator is drawn as `fill_rect(0, NAV_H, w, 1, RULE)`. Where
        it lands is the honest answer to "how tall is the nav", which beats
        reading NAV_H out of a source file that the guest may not have been
        built from.
        """
        m = self.mask(colour, tol=10)
        frac = m.mean(axis=1)
        return [int(y) for y in np.nonzero(frac >= min_frac)[0]]

    def boxes(self, colour, min_w=40, min_h=18, tol=10, side_frac=0.85) -> list[Rect]:
        """Rounded-rect outlines drawn in `colour`.

        `fill_round_rect(border)` then `fill_round_rect(bg)` one pixel in, so
        every card, tile and input field on screen is a one-pixel ring. Collect
        the long horizontal edges, pair each with the *nearest* edge below that
        shares both ends, and insist the sides are drawn between them.

        Two things this has to get right, both learned the hard way:

        * Recent mail rows are separated by 1px `fill_rect` lines with
          identical ends. Without the side check they pair into cards that were
          never drawn, and the harness reports rows that do not exist.
        * A card's bottom edge must be spent once it is used. Left free it
          becomes the "top" of a phantom box reaching down to the next card's
          bottom, and a list of seven rows reads as six of the wrong heights.
        """
        m = self.mask(colour, tol=tol)
        edges: list[tuple[int, int, int]] = []
        for y in range(self.h):
            row = m[y]
            if not row.any():
                continue
            # Bridge small holes. The guest draws its own mouse cursor into
            # the framebuffer, and an arrow parked over a card's edge splits
            # that edge into two short runs - the card then pairs with the
            # *next* card's bottom, and a list of seven rows reads as six of
            # the wrong heights. 12px is wider than the cursor and narrower
            # than the 16px gutter between the home tiles, so it heals the
            # hole without merging neighbours.
            for s, e in _runs(row, 12):
                if e - s + 1 >= min_w:
                    edges.append((y, s, e))
        edges.sort()

        out: list[Rect] = []
        spent: set[int] = set()
        for i, (top, x0, x1) in enumerate(edges):
            if i in spent:
                continue
            for j in range(i + 1, len(edges)):
                if j in spent:
                    continue
                bot, x2, x3 = edges[j]
                if bot - top + 1 < min_h:
                    continue
                if abs(x2 - x0) > 6 or abs(x3 - x1) > 6:
                    continue
                # Skip the corners. Their anti-aliased arc blends the border
                # into the fill, so those rows carry no pixel close enough to
                # the border colour to count - which dragged a perfectly good
                # card down to 0.67 side coverage and made the selected row
                # disappear from the screen entirely.
                margin = max(2, min(_CORNER, (bot - top) // 4))
                band = m[top + margin : bot - margin, :]
                if band.shape[0] <= 0:
                    continue
                # The corner radius pulls the top edge in from the true left of
                # the box, so look for the sides in a window wide enough to
                # hold a corner.
                lo = max(0, min(x0, x2) - _CORNER)
                hi = max(x1, x3)
                left = band[:, lo : min(x0, x2) + 4].any(axis=1).mean()
                right = band[:, max(0, hi - 3) : hi + _CORNER].any(axis=1).mean()
                if left >= side_frac and right >= side_frac:
                    cols = np.nonzero(m[top : bot + 1].any(axis=0))[0]
                    near = cols[(cols >= lo) & (cols <= hi + _CORNER)]
                    rx0 = int(near.min()) if len(near) else x0
                    rx1 = int(near.max()) if len(near) else x1
                    out.append(Rect(rx0, top, rx1 - rx0 + 1, bot - top + 1))
                    spent.add(i)
                    spent.add(j)
                    break

        # One ring can still yield near-identical rectangles when its edges are
        # anti-aliased over two rows. Drop anything mostly inside a box already
        # kept - by area overlap, not by comparing corners, which let four
        # copies of the same row through and made two rows read as four.
        out.sort(key=lambda r: (-r.w * r.h))
        keep: list[Rect] = []
        for r in out:
            if not any(_overlap_frac(r, k) > 0.6 for k in keep):
                keep.append(r)
        keep.sort(key=lambda r: (r.y, r.x))
        return keep

    def solid_blobs(
        self, colour, min_w=60, min_h=16, tol=28, density=0.55, gap=26
    ) -> list[Rect]:
        """Filled areas of `colour` - the accent pill, a toggle track.

        Runs, bridged across gaps up to `gap`, then stitched down the rows
        wherever they overlap. Two shapes of the same colour on one row have to
        stay separate: taking the whole row's first-to-last extent let a single
        anti-aliased pixel of a label, four hundred pixels away, swallow a
        switch - and a screen with seven switches reported one.

        Density then stops a thin outline (a selected row is drawn in accent
        and is far wider than any pill) reading as a filled shape, while still
        tolerating the pill's own label knocked out of it in white.
        """
        m = self.mask(colour, tol=tol)
        open_blobs: list[list[int]] = []  # [x0, x1, top, bot]
        out: list[Rect] = []
        for y in range(self.h):
            row = m[y]
            runs = []
            if row.any():
                for s, e in _runs(row, gap):
                    span = e - s + 1
                    if span >= min_w and row[s : e + 1].sum() >= span * density:
                        runs.append((s, e))
            still: list[list[int]] = []
            for s, e in runs:
                hit = None
                for b in open_blobs:
                    if b[3] == y - 1 and b[1] >= s and b[0] <= e:
                        hit = b
                        break
                if hit is not None:
                    hit[0], hit[1] = min(hit[0], s), max(hit[1], e)
                    hit[3] = y
                    if hit not in still:
                        still.append(hit)
                else:
                    still.append([s, e, y, y])
            for b in open_blobs:
                if b not in still:
                    if b[3] - b[2] + 1 >= min_h:
                        out.append(Rect(b[0], b[2], b[1] - b[0] + 1, b[3] - b[2] + 1))
            open_blobs = still
        for b in open_blobs:
            if b[3] - b[2] + 1 >= min_h:
                out.append(Rect(b[0], b[2], b[1] - b[0] + 1, b[3] - b[2] + 1))
        out.sort(key=lambda r: (r.y, r.x))
        return out

    # --- text -----------------------------------------------------------

    def text_blocks(
        self, fg, region: Rect | None = None, bg=None, gap=11, row_gap=3, min_alpha=45
    ):
        """Separate runs of text of colour `fg` inside `region`.

        Lines first, then words within a line. Splitting on columns alone
        merged a centred heading with the rows stacked underneath it into one
        413x377 blob, and no string can explain a blob - the screen was
        correct and the harness could not read it.
        """
        a = self.alpha(fg, bg)
        r = region or Rect(0, 0, self.w, self.h)
        sub = a[max(0, r.y) : r.bottom, max(0, r.x) : r.right]
        if sub.size == 0:
            return []
        hot = sub >= min_alpha
        blocks = []
        for y0, y1 in _runs(hot.any(axis=1), row_gap):
            band = hot[y0 : y1 + 1]
            for x0, x1 in _runs(band.any(axis=0), gap):
                rows = np.nonzero(band[:, x0 : x1 + 1].any(axis=1))[0]
                blocks.append(
                    Rect(
                        r.x + x0,
                        r.y + y0 + int(rows[0]),
                        x1 - x0 + 1,
                        int(rows[-1] - rows[0]) + 1,
                    )
                )
        return blocks


# Widest corner radius any card is drawn with (`fill_round_rect(.., 12, ..)`).
_CORNER = 14


def _overlap_frac(a: Rect, b: Rect) -> float:
    """Fraction of `a` covered by `b`."""
    ix = max(0, min(a.right, b.right) - max(a.x, b.x))
    iy = max(0, min(a.bottom, b.bottom) - max(a.y, b.y))
    area = a.w * a.h
    return (ix * iy) / area if area else 0.0


def _runs(flags: np.ndarray, gap: int):
    """Maximal `(start, end)` runs of True, merged across blanks up to `gap`."""
    idx = np.nonzero(flags)[0]
    if len(idx) == 0:
        return []
    splits = np.nonzero(np.diff(idx) > gap)[0]
    starts = np.concatenate(([0], splits + 1))
    ends = np.concatenate((splits, [len(idx) - 1]))
    return [(int(idx[s]), int(idx[e])) for s, e in zip(starts, ends)]


def _dice(t: np.ndarray, m: np.ndarray) -> float:
    s = float(t.sum() + m.sum())
    if s <= 0:
        return 0.0
    return float(2.0 * np.minimum(t, m).sum() / s)


def score_text(
    frame: Frame,
    block: Rect,
    text: str,
    face: Face,
    fg,
    bg=None,
    tracking64: int = 0,
    slack: int = 3,
    pad: int | None = None,
) -> float:
    """How well `text`, rendered in `face`, explains the ink in `block`.

    1.0 means every unit of coverage on screen is accounted for by the string
    and vice versa. Anything drawn but not predicted drags it down, so a title
    that gained a word scores low rather than matching its own prefix.
    """
    tpl, tx0, ty0 = face.render(text, tracking64)
    if tpl.size == 0:
        return 0.0
    a = frame.alpha(fg, bg)

    # Compare over a window that covers the whole block, with the candidate
    # painted into it - not over the candidate's own bounding box.
    #
    # Scoring inside the candidate's box makes every prefix perfect: "G"
    # explains the G in "Guided" completely and never has to account for
    # "uided". Reading a screen that way produces confident single letters.
    pad = slack + 2 if pad is None else pad
    wx, wy = block.x - pad, block.y - pad
    ww, wh = block.w + 2 * pad, block.h + 2 * pad
    if wx < 0 or wy < 0 or wx + ww > frame.w or wy + wh > frame.h:
        wx, wy = max(0, wx), max(0, wy)
        ww, wh = min(ww, frame.w - wx), min(wh, frame.h - wy)
    if ww <= 0 or wh <= 0:
        return 0.0
    win = a[wy : wy + wh, wx : wx + ww]

    best = 0.0
    canvas = np.empty((wh, ww), dtype=np.float32)
    for dy in range(-slack, slack + 1):
        for dx in range(-slack, slack + 1):
            canvas.fill(0.0)
            ty, tx = block.y + dy - wy, block.x + dx - wx
            y0, x0 = max(0, ty), max(0, tx)
            y1 = min(wh, ty + tpl.shape[0])
            x1 = min(ww, tx + tpl.shape[1])
            if y1 <= y0 or x1 <= x0:
                continue
            canvas[y0:y1, x0:x1] = tpl[y0 - ty : y1 - ty, x0 - tx : x1 - tx]
            best = max(best, _dice(canvas, win))
    # A candidate longer than the text on screen is not a match, even when the
    # overhang falls outside the comparison window: "nvdaa" scored exactly what
    # "nvda" did, because the spare letter sat where the caret was cropped out.
    if tpl.shape[1] > block.w:
        best *= block.w / tpl.shape[1]
    return best


def best_of(frame: Frame, block: Rect, candidates, face: Face, fg, bg=None, tracking64=0):
    """The candidate string that best explains `block`, with its score."""
    scored = [
        (score_text(frame, block, c, face, fg, bg, tracking64), c) for c in candidates
    ]
    scored.sort(reverse=True)
    return scored[0] if scored else (0.0, None)


ALPHABET = [chr(c) for c in range(0x20, 0x7F)]

# Enough of the alphabet to tell one cut from another without paying for all
# of it. Letters only: punctuation is too small to distinguish 13px from 17px.
_PROBE = [chr(c) for c in range(ord("A"), ord("Z") + 1)] + [
    chr(c) for c in range(ord("a"), ord("z") + 1)
]


def face_fit(frame: Frame, block: Rect, face: Face, fg, bg=None) -> float:
    """How well any single glyph of `face` lands on the start of `block`.

    Used to choose the cut a line was set in. Choosing by ink height does not
    work - "Email" in a 17px face is 10px tall because it has no descender,
    which puts it closer to the 13px cut on paper and produces a confident
    read of "B}ndtI".
    """
    a = frame.alpha(fg, bg)
    best = 0.0
    for baseline in range(block.y + 2, block.y + face.ascent + 5, 2):
        for x in range(block.x - 1, block.x + 2):
            for ch in _PROBE:
                g = face.glyph_for(ord(ch))
                if g.w == 0 or g.h == 0:
                    continue
                cell, dx, dy = face.cell(ord(ch))
                gy, gx = baseline + dy, x + dx
                if gy < 0 or gx < 0 or gy + cell.shape[0] > frame.h or gx + cell.shape[1] > frame.w:
                    continue
                s = _dice(cell, a[gy : gy + cell.shape[0], gx : gx + cell.shape[1]])
                if s > best:
                    best = s
    return best



def ocr(
    frame: Frame,
    block: Rect,
    face: Face,
    fg,
    bg=None,
    tracking64: int = 0,
    beam: int = 6,
    max_len: int = 64,
    floor: float = 0.55,
) -> str:
    """Read text of unknown content out of `block`.

    Beam search over the alphabet, advancing the pen exactly as `draw_text`
    does. Greedy does not work: a lowercase `l` sits perfectly inside the stem
    of an `h`, scores 1.0 on its own, and only reveals itself as wrong when the
    next glyph lands in the middle of the bowl. Keeping several hypotheses
    alive lets that later evidence settle it.

    Used for row titles, where the point is comparing two rows against each
    other - the same document listed twice under two labels reads as the same
    string whether or not every glyph was named correctly.
    """
    a = frame.alpha(fg, bg)

    def glyph_score(ch, pen_px, baseline):
        g = face.glyph_for(ord(ch))
        if g.w == 0 or g.h == 0:
            return None, g
        cell, dx, dy = face.cell(ord(ch))
        gx, gy = pen_px + dx, baseline + dy
        if gx < 0 or gy < 0 or gy + cell.shape[0] > frame.h or gx + cell.shape[1] > frame.w:
            return None, g
        return _dice(cell, a[gy : gy + cell.shape[0], gx : gx + cell.shape[1]]), g

    # Calibrate the pen before searching. The baseline is only recoverable to
    # within a pixel or two from the ink box - the tallest glyph in a string is
    # rarely the tallest in the face - and running the full beam for every
    # candidate origin costs more than the read is worth.
    # The baseline sits somewhere between the block's top and one face-ascent
    # below it: how far depends on which glyphs the string happens to use. A
    # string of cap-height letters tops out well short of the ascender line, so
    # a tight window around `block.y + ascent` misses the true pen by a pixel
    # and every glyph after it reads as its nearest lookalike - "Search" came
    # back as "6oarch" from being one row low.
    origins = []
    for baseline in range(block.y + 2, block.y + face.ascent + 5):
        for x_start in range(block.x - 3, block.x + 4):
            best = 0.0
            for ch in ALPHABET:
                s, _ = glyph_score(ch, x_start, baseline)
                if s and s > best:
                    best = s
            origins.append((best, baseline, x_start))
    origins.sort(reverse=True)

    span = block.w + 8
    cands: set[str] = set()
    for _, baseline, x_start in origins[:3]:
        beams = [(0.0, 0, "", x_start * 64)]
        for _ in range(min(max_len, span // 3 + 2)):
            nxt = []
            for total, n, txt, pen64 in beams:
                pen64e = pen64 + (tracking64 if txt else 0)
                pen_px = (pen64e + 32) >> 6
                if pen_px > block.right + 2:
                    cands.add(txt)
                    continue
                ranked = []
                for ch in ALPHABET:
                    s, g = glyph_score(ch, pen_px, baseline)
                    if s is not None and s >= floor:
                        ranked.append((s, ch, g))
                # A space has no bitmap, so it cannot be scored by overlap.
                # Score it by how empty its advance is - graded, not a
                # threshold. An all-or-nothing "is this band blank" test broke
                # the chain wherever a neighbouring glyph's anti-aliased tail
                # leaned into the gap, and every word after that point came out
                # as nonsense.
                sp = face.glyph_for(ord(" "))
                adv = max((sp.adv64 + 32) >> 6, 1)
                band = a[
                    max(0, baseline - face.ascent) : baseline - face.descent,
                    pen_px : pen_px + adv,
                ]
                if band.size:
                    s_sp = max(0.0, 1.0 - float(band.mean()) / 25.0)
                    if s_sp >= floor:
                        ranked.append((s_sp, " ", sp))
                if not ranked:
                    cands.add(txt)
                    continue
                ranked.sort(reverse=True, key=lambda r: r[0])
                for s, ch, g in ranked[:beam]:
                    nxt.append((total + s, n + 1, txt + ch, pen64e + g.adv64))
            if not nxt:
                break
            nxt.sort(reverse=True, key=lambda b: b[0] / max(b[1], 1))
            beams = nxt[:beam]
            cands.update(b[2] for b in beams)
        cands.update(b[2] for b in beams)

    # Rank by how much of the block each hypothesis explains, not by how well
    # its own glyphs scored: a prefix scores perfectly on every glyph it has
    # and leaves the rest of the word unaccounted for.
    best_txt, best_score = "", -1.0
    for txt in cands:
        t = txt.strip()
        if not t:
            continue
        sc = score_text(frame, block, t, face, fg, bg, tracking64, slack=3)
        if sc > best_score or (sc == best_score and len(t) > len(best_txt)):
            best_txt, best_score = t, sc
    return best_txt
