"""The kernel's own font atlas, loaded on the host so the harness can read
text off a screenshot.

`kernel/build.rs` rasterises Inter into `$OUT_DIR/font_atlas.rs`: one alpha
bitmap and one glyph table per face. The kernel blends those alphas onto the
framebuffer and nothing else. That makes rendering exactly reproducible here,
which turns "what does the screen say" from OCR guesswork into arithmetic: we
re-render a candidate string with the same glyph table and the same 1/64px pen
and compare coverage.

Reading the atlas rather than shipping a copy is the point. When the other
session changes a font size, this file follows it on the next build instead of
matching yesterday's pixels and calling the product broken.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent.parent

FIRST, LAST = 0x20, 0x7E


@dataclass(frozen=True)
class Glyph:
    w: int
    h: int
    bx: int
    by: int
    adv64: int
    off: int


class Face:
    """One size+weight cut, with the same geometry the kernel uses."""

    def __init__(self, name, glyphs, bitmap, ascent, descent, line, px):
        self.name = name
        self.glyphs = glyphs
        self.bitmap = bitmap
        self.ascent = ascent
        self.descent = descent
        self.line = line
        self.px = px
        self._tpl: dict[int, np.ndarray] = {}

    def __repr__(self):
        return f"<Face {self.name} px={self.px}>"

    def glyph_for(self, ch: int) -> Glyph:
        if not (FIRST <= ch <= LAST):
            ch = ord("?")
        return self.glyphs[ch - FIRST]

    def template(self, ch: int) -> np.ndarray:
        """Alpha bitmap for one byte, as float (h, w)."""
        if ch not in self._tpl:
            g = self.glyph_for(ch)
            if g.w == 0 or g.h == 0:
                self._tpl[ch] = np.zeros((0, 0), dtype=np.float32)
            else:
                flat = self.bitmap[g.off : g.off + g.w * g.h]
                self._tpl[ch] = flat.reshape(g.h, g.w).astype(np.float32)
        return self._tpl[ch]

    def cell(self, ch: int):
        """Glyph coverage padded out to its whole line cell.

        Returns `(alpha, dx, dy)` for a box spanning the glyph's advance width
        and the face's full ascent-to-descent band, so a comparison against the
        screen counts ink that is *there but not predicted* as a miss.

        Comparing bare glyph bitmaps does not: a full stop is a 2x2 blob that
        sits perfectly inside the stroke of a G and scores 1.0. Reading a
        screen one bare glyph at a time therefore spells confident nonsense.
        """
        key = ("cell", ch)
        if key not in self._tpl:
            g = self.glyph_for(ch)
            adv = (g.adv64 + 32) >> 6
            x0 = min(0, g.bx)
            x1 = max(adv, g.bx + g.w, x0 + 1)
            y0, y1 = -self.ascent, -self.descent
            buf = np.zeros((y1 - y0, x1 - x0), dtype=np.float32)
            if g.w and g.h:
                t = self.template(ch)
                r, c = g.by - y0, g.bx - x0
                if r >= 0 and c >= 0 and r + g.h <= buf.shape[0] and c + g.w <= buf.shape[1]:
                    buf[r : r + g.h, c : c + g.w] = t
                else:  # a glyph that overshoots the face band (rare)
                    buf = np.zeros((max(y1 - y0, g.h), max(x1 - x0, g.w)), dtype=np.float32)
                    buf[: g.h, : g.w] = t
                    y0 = g.by
                    x0 = g.bx
            self._tpl[key] = (buf, x0, y0)
        return self._tpl[key]

    def width(self, text: str, tracking64: int = 0) -> int:
        total = sum(self.glyph_for(ord(c)).adv64 for c in text)
        n = len(text)
        if n > 1:
            total += tracking64 * (n - 1)
        return (total + 32) >> 6

    def place(self, text: str, tracking64: int = 0):
        """Glyph boxes for `text`, relative to (pen x = 0, baseline = 0).

        Mirrors `Surface::draw_text`: the pen runs in 1/64px and each glyph
        lands at `((pen64 + 32) >> 6) + bx`, so fractional advances round the
        same way they do in the kernel. Doing this in whole pixels drifts by a
        pixel or two across a word, and a one-pixel drift is enough to make a
        correct render score like a mismatch.
        """
        out = []
        pen64 = 0
        for i, c in enumerate(text):
            if i:
                pen64 += tracking64
            g = self.glyph_for(ord(c))
            pen_px = (pen64 + 32) >> 6
            out.append((pen_px + g.bx, g.by, g, ord(c)))
            pen64 += g.adv64
        return out, (pen64 + 32) >> 6

    def render(self, text: str, tracking64: int = 0):
        """Alpha coverage for `text`.

        Returns `(alpha, x0, y0)` where `alpha[r][c]` is the coverage at
        framebuffer `(pen_x + x0 + c, baseline + y0 + r)`.
        """
        boxes, _ = self.place(text, tracking64)
        ink = [(gx, gy, g) for gx, gy, g, _ in boxes if g.w and g.h]
        if not ink:
            return np.zeros((0, 0), dtype=np.float32), 0, 0
        x0 = min(gx for gx, _, _ in ink)
        y0 = min(gy for _, gy, _ in ink)
        x1 = max(gx + g.w for gx, _, g in ink)
        y1 = max(gy + g.h for _, gy, g in ink)
        buf = np.zeros((y1 - y0, x1 - x0), dtype=np.float32)
        for gx, gy, g in ink:
            t = self.bitmap[g.off : g.off + g.w * g.h].reshape(g.h, g.w).astype(np.float32)
            r, c = gy - y0, gx - x0
            view = buf[r : r + g.h, c : c + g.w]
            np.maximum(view, t, out=view)
        return buf, x0, y0


_NUM = re.compile(rb"-?\d+")
_GLYPH = re.compile(
    r"Glyph \{ w: (-?\d+), h: (-?\d+), bx: (-?\d+), by: (-?\d+), adv64: (-?\d+), off: (-?\d+) \}"
)
_FACE = re.compile(
    r"pub static (\w+)_FACE: Face = Face \{ glyphs: &\w+, bitmap: &\w+, "
    r"ascent: (-?\d+), descent: (-?\d+), line: (-?\d+), px: (-?\d+) \}"
)


def atlas_path(root: Path = ROOT) -> Path:
    """Newest generated atlas under `target/`.

    Cargo keeps one OUT_DIR per build-script fingerprint, so a repo that has
    been built more than once has several. The newest is the one the current
    ISO was made from; an older one is a different font and every match here
    would fail for reasons that have nothing to do with the UI.
    """
    cands = sorted(
        (root / "target").rglob("build/*/out/font_atlas.rs"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    if not cands:
        raise RuntimeError(
            "no font_atlas.rs under target/ - build the kernel first (make iso)"
        )
    return cands[0]


def load(path: Path | None = None) -> dict[str, Face]:
    """Parse the generated atlas into faces keyed by name (HERO, BRAND, ...)."""
    path = path or atlas_path()
    src = path.read_text()

    bitmaps: dict[str, np.ndarray] = {}
    for m in re.finditer(r"pub static (\w+)_BITMAP: \[u8; (\d+)\] = \[", src):
        name, n = m.group(1), int(m.group(2))
        end = src.index("];", m.end())
        body = src[m.end() : end].encode()
        vals = np.array(_NUM.findall(body), dtype=np.int64).astype(np.uint8)
        if len(vals) != n:
            raise RuntimeError(f"{name}_BITMAP: expected {n} bytes, parsed {len(vals)}")
        bitmaps[name] = vals

    glyphs: dict[str, list[Glyph]] = {}
    for m in re.finditer(r"pub static (\w+)_GLYPHS: \[Glyph; (\d+)\] = \[", src):
        name, n = m.group(1), int(m.group(2))
        end = src.index("];", m.end())
        gs = [Glyph(*(int(v) for v in g)) for g in _GLYPH.findall(src[m.end() : end])]
        if len(gs) != n:
            raise RuntimeError(f"{name}_GLYPHS: expected {n}, parsed {len(gs)}")
        glyphs[name] = gs

    faces: dict[str, Face] = {}
    for name, asc, desc, line, px in _FACE.findall(src):
        faces[name] = Face(
            name, glyphs[name], bitmaps[name], int(asc), int(desc), int(line), int(px)
        )
    if not faces:
        raise RuntimeError(f"parsed no faces out of {path}")
    return faces


def tracking_pct(px: int, pct_tenths: int) -> int:
    """`font::tracking_pct` - letter-spacing in 1/64px.

    Rust's `/` truncates toward zero; Python's `//` floors. On negative
    tracking - which every display string here uses - those differ by one
    64th of a pixel per gap, and over a nineteen-character heading that is
    enough drift to shift a glyph and score a correct render as a miss.
    """
    n = px * 64 * pct_tenths
    return int(n / 1000) if n < 0 else n // 1000
