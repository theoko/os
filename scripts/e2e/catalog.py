"""What screens exist, read out of the kernel source.

The nav title strip is the guest telling you where you are, so the harness only
needs the list of things it might say. That list is in `screens.rs` as literal
arguments to `chrome()`, in `searchui.rs` as the one centred BRAND string, and
in `setup.rs` as the arguments to `header()`.

Scraping them is deliberate. This project added a Status screen and a Brief
screen in one week; a hand-kept list in the harness would have called both of
them "unknown screen" and the run would have failed as if the product were
broken. Scraped, they turn up on the next run for free.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
SRC = ROOT / "kernel" / "src"

# Nav titles that the driver knows how to reach and what they mean. Anything
# scraped but not listed here is still identified - it just has no route.
_SLUG = {
    "Search": "search",
    "Skills": "skills",
    "Brief": "brief",
    "Capabilities": "caps",
    "Status": "status",
    "Portal": "portal",
    "Playbook": "playbook",
}


def _calls(src: str, fn: str) -> list[str]:
    """Every `fn(...)` call body, paren-balanced so multi-line calls survive."""
    out = []
    for m in re.finditer(re.escape(fn) + r"\(", src):
        depth, i = 1, m.end()
        while i < len(src) and depth:
            if src[i] == "(":
                depth += 1
            elif src[i] == ")":
                depth -= 1
            i += 1
        out.append(src[m.end() : i - 1])
    return out


def app_titles(src: Path = SRC) -> list[str]:
    """Centred nav-strip titles on the post-setup screens."""
    out = []
    screens = (src / "screens.rs").read_text()
    out += re.findall(r'chrome\(\s*fb,\s*w,\s*"([^"]+)"', screens)
    # searchui draws its own chrome; the BRAND-face centred string is the title.
    su = (src / "searchui.rs").read_text()
    for call in _calls(su, "draw_text_centered"):
        if "BRAND_FACE" in call:
            lit = re.search(r'"([^"]+)"', call)
            if lit:
                out.append(lit.group(1))
    seen, uniq = set(), []
    for t in out:
        if t not in seen:
            seen.add(t)
            uniq.append(t)
    return uniq


def home_brand(src: Path = SRC) -> str:
    """The wordmark `draw_nav` puts at the left of the home nav strip."""
    ui = (src / "ui.rs").read_text()
    m = re.search(r'fn draw_nav\(.*?draw_text\(\s*PAD_X,\s*base,\s*"([^"]+)"', ui, re.S)
    return m.group(1) if m else "os"


def setup_headings(src: Path = SRC) -> list[tuple[str, str]]:
    """`(step slug, heading)` for every setup step that draws a heading."""
    s = (src / "setup.rs").read_text()
    out: list[tuple[str, str]] = []

    def _fn(name):
        m = re.search(rf"fn {name}\(.*?\n    \}}", s, re.S)
        return m.group(0) if m else ""

    for fn in re.findall(r"fn draw_(\w+)\(&mut self, fb: &Surface", s):
        body = _fn(f"draw_{fn}")
        if not body:
            continue
        m = re.search(r'self\.header\(\s*fb,\s*w,\s*h,\s*"([^"]+)"', body, re.S)
        if m:
            out.append((fn, m.group(1)))
            continue
        # Welcome and Done have no header(); they centre their own display type.
        m = re.search(r'draw_text_centered\(w / 2, cy, "([^"]+)"', body)
        if m:
            out.append((fn, m.group(1)))
    return out


def setup_face(src: Path = SRC) -> dict[str, str]:
    """Which face each setup heading is drawn in, so scoring uses the right cut."""
    s = (src / "setup.rs").read_text()
    faces = {}
    for fn, heading in setup_headings(src):
        m = re.search(rf"fn draw_{fn}\(.*?\n    \}}", s, re.S)
        body = m.group(0) if m else ""
        if "HERO_FACE" in body and "cy," in body:
            faces[heading] = "HERO"
        else:
            faces[heading] = "TITLE"
    return faces


def slug_for(title: str) -> str:
    return _SLUG.get(title, "screen:" + title.lower().replace(" ", "-"))


if __name__ == "__main__":
    print("home brand :", home_brand())
    print("app titles :", app_titles())
    print("setup      :", setup_headings())
    print("faces      :", setup_face())
