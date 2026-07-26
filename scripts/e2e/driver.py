#!/usr/bin/env python3
"""Drive the real x86 ISO in QEMU by reading the screen, not by remembering it.

The previous driver walked a fixed journey - click Continue six times, then
click (640, 158) - and every UI change broke it in a way that looked like a
product bug. The placeholder changed, Enter started running an agent Brief
instead of a search, Status and Brief screens appeared, the capability rows
moved twice, setup got shorter. None of those were defects. All of them turned
the harness red.

So this one derives its state from the framebuffer every step:

    identify()      which screen is showing, from the nav title strip
    find_control()  where the primary action / Back / field / rows are, by
                    what they look like
    navigate_to()   get from wherever you are to a named screen, or fail
                    loudly with the screenshot that shows where it stuck

and then asserts things that can only be checked with the whole screen in
front of you: that a control which is drawn does something, that the number in
the prose matches the number of cards below it, that the same document is not
listed twice, that the query arrived intact.

    ./scripts/e2e/driver.py --out /tmp/x --query nvda
"""

from __future__ import annotations

import argparse
import json
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))
    __package__ = "scripts.e2e"

from . import atlas, screen, theme  # noqa: E402
from .frame import Frame, Rect, score_text, to_png, write_ppm  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent.parent

# Keystrokes QEMU knows by name; anything else has to be spelled out.
KEYMAP = {
    " ": "spc",
    ".": "dot",
    ",": "comma",
    "-": "minus",
    "/": "slash",
    "?": "shift-slash",
    "'": "apostrophe",
}


class Qmp:
    """Minimal QMP client. Enough to move a mouse and take a picture."""

    def __init__(self, path, width=0, height=0):
        self.width, self.height = width, height
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        for _ in range(120):
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
        """Absolute move. usb-tablet reports in a 0..32767 space."""
        if not self.width:
            raise RuntimeError("framebuffer size unknown - capture a frame first")
        self.cmd(
            "input-send-event",
            events=[
                {"type": "abs", "data": {"axis": "x", "value": x * 32767 // self.width}},
                {"type": "abs", "data": {"axis": "y", "value": y * 32767 // self.height}},
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
        """Type at roughly human speed.

        At 0.04s a key this dropped characters - "i wanna work on my paper"
        arrived as "i wanna wy paper". 25 keys a second is 300 words a minute,
        which says more about the harness than the kernel, so slow down and
        keep the test about the product.
        """
        for ch in text:
            if ch in KEYMAP:
                self.key(KEYMAP[ch])
            elif ch.isalnum():
                self.key(ch)
            else:
                raise ValueError(f"no key mapping for {ch!r}")
            time.sleep(delay)

    def shot(self, path):
        self.cmd("screendump", filename=str(path))


class Stuck(RuntimeError):
    """Navigation gave up. Carries the screenshot that shows where."""

    def __init__(self, msg, shot: Path | None):
        super().__init__(f"{msg} (see {shot})" if shot else msg)
        self.shot = shot


class Session:
    """A booted guest plus the screen-reading layer over it."""

    def __init__(self, qmp: Qmp, out: Path, faces, keep_ppm=False, verbose=True):
        self.qmp = qmp
        self.out = out
        self.faces = faces
        self.keep_ppm = keep_ppm
        self.verbose = verbose
        self.shots: list[Path] = []
        self.n = 0

    def log(self, msg):
        if self.verbose:
            print(msg)

    def park(self):
        """Get the pointer out of the way before photographing the screen.

        The guest draws its own cursor into the framebuffer. Parked over the
        "os" wordmark it corrupted exactly the pixels the nav strip is read
        from, and a perfectly ordinary home screen came back as "unknown" -
        the harness reporting a product bug that was its own arrow.
        """
        if self.qmp.width:
            self.qmp.move(self.qmp.width - 3, self.qmp.height - 3)
            time.sleep(0.25)

    def capture(self, name=None) -> Frame:
        self.n += 1
        name = f"{self.n:02d}-{name or 'frame'}"
        ppm = self.out / f"{name}.ppm"
        png = self.out / f"{name}.png"
        if not self.qmp.width:
            probe = self.out / "_size.ppm"
            self.qmp.shot(probe)
            sized = Frame.load(probe)
            self.qmp.width, self.qmp.height = sized.w, sized.h
            probe.unlink(missing_ok=True)
            self.log(f"  framebuffer: {sized.w}x{sized.h}")
        self.park()
        self.qmp.shot(ppm)
        f = Frame.load(ppm)
        to_png(ppm, png)
        if not self.keep_ppm:
            ppm.unlink(missing_ok=True)
        else:
            f.path = ppm
        self.shots.append(png)
        return f

    # --- screen-derived navigation ------------------------------------

    def where(self, f: Frame | None = None, name=None):
        f = f if f is not None else self.capture(name)
        return screen.identify(f, self.faces), f

    def find(self, f: Frame, kind: str):
        return screen.find_control(f, kind, self.faces)

    def click(self, ctl, settle=2.2):
        x, y = ctl.centre if hasattr(ctl, "centre") else ctl
        self.qmp.click(x, y)
        time.sleep(settle)

    def _step_toward(self, f: Frame, here, target: str):
        """One action that should move `here` closer to `target`.

        Deliberately small: the only knowledge encoded is "setup advances
        through its primary action", "Back gets you home", and "home's
        destinations are its tiles and its status dot". Everything else -
        which tile, where it is, what it is called - is read off the screen.
        """
        if here.name.startswith("setup:"):
            p = self.find(f, "primary")
            if p is None:
                raise Stuck(f"{here.name} has no primary action to advance", f.path)
            self.log(f"  {here.name}: primary {p.label or '(unlabelled)'} at {p.centre}")
            return p, 3.0

        if here.name != "home":
            b = self.find(f, "back")
            if b is None:
                raise Stuck(f"{here.name} has no Back link; cannot reach {target}", f.path)
            self.log(f"  {here.name}: Back at {b.centre}")
            return b, 2.2

        if target == "status":
            d = self.find(f, "status_dot")
            if d is None:
                raise Stuck("home draws no status dot", f.path)
            return d, 2.5

        want = {"search": "Search", "caps": "Capabilities", "skills": "Skills"}.get(target)
        if want:
            for t in self.find(f, "tiles"):
                if t.label.lower().startswith(want.lower()[:5]):
                    self.log(f"  home: tile {t.label!r} at {t.centre}")
                    return t, 2.5
            labels = [t.label for t in self.find(f, "tiles")]
            raise Stuck(f"home has no tile for {target!r}; tiles read as {labels}", f.path)

        raise Stuck(f"no route from home to {target!r}", f.path)

    def navigate_to(self, target: str, limit=12) -> Frame:
        """Get to `target`, or fail saying exactly where it stopped."""
        seen = []
        f = self.capture(f"nav-{target}")
        for _ in range(limit):
            here, _ = self.where(f)
            self.log(f"  at {here}")
            if here.name == target:
                return f
            seen.append(here.name)
            if len(seen) >= 3 and seen[-1] == seen[-2] == seen[-3]:
                raise Stuck(
                    f"stuck on {here.name} while heading for {target}; "
                    f"path was {' -> '.join(seen)}",
                    f.path,
                )
            ctl, settle = self._step_toward(f, here, target)
            self.click(ctl, settle)
            f = self.capture(f"nav-{target}")
        here, _ = self.where(f)
        raise Stuck(
            f"gave up after {limit} steps heading for {target}; ended on {here.name} "
            f"(path {' -> '.join(seen)})",
            f.path,
        )


class Checks:
    """Collected assertions, reported together so one failure hides no others."""

    def __init__(self):
        self.results = []

    def that(self, name, ok, detail=""):
        # Detail only on failure. Printing the failure message next to a PASS
        # reads as "PASS - still on home after clicking", which is the opposite
        # of what happened, and a transcript nobody can read is a transcript
        # nobody checks.
        self.results.append((bool(ok), name, detail))
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f" - {detail}"))
        return bool(ok)

    def skip(self, name, why):
        print(f"  SKIP  {name} - {why}")

    def report(self):
        failed = [r for r in self.results if not r[0]]
        print(f"\n{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        for _, name, detail in failed:
            print(f"  FAILED: {name} - {detail}")
        return 1 if failed else 0


# --- assertions that need the whole screen ------------------------------


def check_layout(f: Frame, faces, checks: Checks, where: str):
    """Every control drawn is reachable, and none sits on top of another.

    Both halves have shipped: a "Go Back" link drawn below the bottom edge, and
    a footer laid over the last capability row so its switch could not be hit.
    """
    off = screen.offscreen(f, faces)
    checks.that(
        f"{where}: every control is inside the framebuffer", not off, ", ".join(off)
    )
    clash = screen.clashes(f, faces)
    checks.that(
        f"{where}: nothing is drawn on top of anything else", not clash, "; ".join(clash)
    )


def check_destinations_are_wired(s: Session, checks: Checks):
    """Click each home destination and prove the screen actually changes.

    This is the bug class the project keeps shipping: a control that is drawn,
    looks clickable, and does nothing. The recent-mail rows did it; the portal
    pill did it. Nothing short of clicking it and looking finds that.
    """
    f = s.navigate_to("home")
    dests = [(t.label or "tile", t) for t in s.find(f, "tiles")]
    dot = s.find(f, "status_dot")
    if dot:
        dests.append(("status dot", dot))
    if not dests:
        checks.that("home draws at least one destination", False, "no tiles, no status dot")
        return
    for label, ctl in dests:
        s.click(ctl, 2.5)
        after = s.capture(f"wired-{label.replace(' ', '-').lower()}")
        here, _ = s.where(after)
        checks.that(
            f"home destination {label!r} goes somewhere",
            here.name != "home",
            f"still on home after clicking {ctl.rect}",
        )
        if here.name != "home":
            b = s.find(after, "back")
            if b is None:
                checks.that(
                    f"{here.name} (from {label!r}) offers a way back",
                    False,
                    "no Back link - this screen is a dead end",
                )
                f = s.navigate_to("home")
                continue
            checks.that(f"{here.name} (from {label!r}) offers a way back", True)
            s.click(b, 2.2)
        f = s.capture("wired-home")
        back_home, _ = s.where(f)
        if back_home.name != "home":
            f = s.navigate_to("home")


def frame_score(f: Frame, fld, text: str, faces) -> float:
    """Coverage overlap between `text` rendered into the field and what is drawn.

    Reading the field back letter by letter answers "what does it say"; this
    answers "does it say this", which is the actual question when a keystroke
    may have been dropped, and it does not depend on every glyph being named
    correctly.
    """
    inner = screen._no_caret(
        f, f.text_blocks(theme.INK, fld.rect.inset(6), min_alpha=screen.INK_LEVEL)
    )
    if not inner:
        return 0.0
    x0 = min(b.x for b in inner)
    y0 = min(b.y for b in inner)
    blk = Rect(x0, y0, max(b.right for b in inner) - x0, max(b.bottom for b in inner) - y0)
    # Tight window. The default padding reaches far enough to swallow the
    # insertion caret sitting a few pixels past the last glyph, and unexplained
    # ink inside the window costs score - correct text capped out at 0.91 and a
    # spurious extra letter scored the same, because the extra letter was
    # covering the caret.
    return max(
        score_text(f, blk, text, fc, theme.INK, slack=2, pad=1)
        for fc in (faces["BODY"], faces["BRAND"])
    )


def norm(text: str) -> str:
    return " ".join(text.lower().split())


# Brief rows that describe the run rather than report a finding. Everything
# else is a result, and a result must not appear twice under any label.
COMMENTARY_TAGS = {"goal", "query", "info", "plan", "next", "blocked", "need"}


def duplicate_results(titles, kinds):
    """Rows naming the same thing twice in a way the screen should not.

    The arm64 guest listed "Doc: os identity" and "Hit: os identity" in one
    report - one document, twice, under two labels. But a Brief legitimately
    echoes the same string under two *different* commentary labels ("Goal:
    nvda" above "Query: nvda"), so flagging any repeat is wrong.

    The line is drawn on RESULT vs COMMENTARY, not on openability.

    An earlier version required both rows to be openable, or their labels to
    match. That could not catch the bug it was written for: screens.rs:257
    renders the tag in ACCENT only for Urgent|Reply|Need|Event|Doc, so "Doc"
    reads openable and "Hit" does not - Doc/Hit satisfied neither arm, and the
    check passed on the exact defect the arm64 guest was displaying. This
    mirrors Brief::is_result in kernel/src/agent.rs, which draws the same line.
    """
    dupes = []
    for i in range(len(titles)):
        for j in range(i + 1, len(titles)):
            if not titles[i].strip() or norm(titles[i]) != norm(titles[j]):
                continue
            ki = kinds[i][0] if i < len(kinds) else ""
            kj = kinds[j][0] if j < len(kinds) else ""
            # Commentary may legitimately echo: "Goal: nvda" over "Query: nvda".
            if norm(ki) in COMMENTARY_TAGS and norm(kj) in COMMENTARY_TAGS:
                continue
            dupes.append(f"{ki or '?'}/{kj or '?'}: {titles[i]!r}")
    return dupes


class BridgeWitness:
    """Only what the bridge logged during *this* run counts as evidence.

    Twice now a run has been graded against a `.bridge.log` that still held a
    previous session's traffic, and reported "the query reached the bridge" on
    a guest whose COM2 was never connected. The bridge is also owned by another
    session that rebuilds and restarts it, which silently drops QEMU's TCP
    chardev - so its identity is recorded too, and a run that spanned a restart
    says so instead of blaming the UI.
    """

    def __init__(self, root: Path):
        self.log = root / ".bridge.log"
        self.pidfile = root / ".bridge.pid"
        self.offset = self.log.stat().st_size if self.log.exists() else 0
        self.pid = self._pid()

    def _pid(self):
        try:
            return self.pidfile.read_text().strip()
        except OSError:
            return ""

    def restarted(self) -> bool:
        return self._pid() != self.pid

    def tail(self) -> str:
        if not self.log.exists():
            return ""
        with self.log.open("rb") as fh:
            # A restart truncates the log, so a shorter file means start over.
            if self.log.stat().st_size >= self.offset:
                fh.seek(self.offset)
            return fh.read().decode(errors="replace")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="/tmp/os-ui")
    ap.add_argument("--iso", default=str(ROOT / "os.iso"))
    ap.add_argument(
        "--bridge",
        default="127.0.0.1:7420",
        help="host MCP bridge for COM2; empty string to run offline",
    )
    ap.add_argument("--query", default="i wanna work on my paper")
    ap.add_argument("--grant", default="2,5", help="capability rows to switch on (0-indexed)")
    ap.add_argument("--boot-wait", type=float, default=14.0)
    ap.add_argument("--keep-ppm", action="store_true", help="keep raw frames for selftest")
    ap.add_argument(
        "--goto",
        help="boot, navigate to this screen, and stop - for checking a route by hand",
    )
    args = ap.parse_args()

    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    faces = atlas.load()
    dot_state = "unknown"
    print(f"atlas: {atlas.atlas_path()}")
    print(f"theme: accent={theme.ACCENT} card={theme.CARD_BORDER} rule={theme.RULE}")

    if args.bridge:
        subprocess.run(
            [str(ROOT / "scripts" / "ensure-bridge.sh")], cwd=ROOT, check=True, capture_output=True
        )
        time.sleep(1.5)

    # After ensure-bridge, never before. It restarts the bridge whenever the
    # binary is newer - which it often is, because another session owns that
    # code - and a witness taken first sees its own start-up as a mid-run
    # restart and fails the run for it.
    witness = BridgeWitness(ROOT)

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
            "-device", "piix3-usb-uhci,id=uhci0",
            "-device", "usb-tablet,bus=uhci0.0",
            "-qmp", f"unix:{qmp_path},server=on,wait=off",
            "-no-reboot",
        ]
        + (["-serial", f"tcp:{args.bridge},server=off"] if args.bridge else []),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
    )

    checks = Checks()
    rc = 1
    s = None
    try:
        qmp = Qmp(str(qmp_path))
        print("booting")
        time.sleep(args.boot_wait)
        s = Session(qmp, out, faces, keep_ppm=True)

        f = s.capture("boot")
        here, _ = s.where(f)
        print(f"  first screen: {here}")

        if args.goto:
            f = s.navigate_to(args.goto)
            here, _ = s.where(f)
            print(f"\narrived: {here}")
            for kind, r in screen.controls(f, faces):
                print(f"  {kind:11s} {r}")
            return 0

        # --- walk setup by reading it ---------------------------------
        print("\nsetup:")
        did_grants = False
        for _ in range(14):
            here, _ = s.where(f)
            if not here.name.startswith("setup:"):
                break
            print(f"  at {here}")
            check_layout(f, faces, checks, here.name)

            if here.name == "setup:caps" and args.grant and not did_grants:
                did_grants = True
                want = [int(v) for v in args.grant.split(",") if v.strip()]
                sw = s.find(f, "toggles")
                print(f"  {len(sw)} switches: {[c.label for c in sw]}")
                checks.that(
                    "every capability row draws a switch",
                    len(sw) == len(s.find(f, "rows")),
                    f"{len(s.find(f, 'rows'))} rows but {len(sw)} switches - "
                    f"a row you cannot grant is a row that does nothing",
                )
                for i in want:
                    if i >= len(sw):
                        checks.that(
                            f"capability row {i} exists to grant",
                            False,
                            f"only {len(sw)} switches drawn",
                        )
                        continue
                    if sw[i].label == "on":
                        print(f"  switch {i} already on")
                        continue
                    print(f"  granting switch {i} at {sw[i].centre}")
                    # Granting makes the guest call the host to build that
                    # source's index and it blocks the UI loop while it waits;
                    # clicks that land during the wait are dropped.
                    s.click(sw[i], 6.0)
                f = s.capture("granted")
                after = s.find(f, "toggles")
                asked = [i for i in want if i < len(after)]
                checks.that(
                    "every switch we asked for reads back on",
                    all(after[i].label == "on" for i in asked),
                    f"asked {asked}, screen shows {[c.label for c in after]}",
                )

            p = s.find(f, "primary")
            if p is None:
                checks.that(
                    f"{here.name} offers a way forward",
                    False,
                    "no primary action drawn - this step is a dead end",
                )
                break
            s.click(p, 3.0)
            f = s.capture("setup")

        # --- home -----------------------------------------------------
        print("\nhome:")
        f = s.navigate_to("home")
        check_layout(f, faces, checks, "home")
        fld = s.find(f, "field")
        checks.that("home draws a text field", fld is not None, "no RULE-outlined input box")
        if fld:
            print(f"  field {fld.rect} placeholder={fld.label!r}")
            checks.that(
                "the field carries a placeholder",
                bool(fld.label.strip()),
                "field is drawn but empty - nothing tells you what it is for",
            )

        # --- every drawn destination must do something ----------------
        print("\nwiring:")
        check_destinations_are_wired(s, checks)

        # --- clicking the field must not restart setup ----------------
        print("\nquery:")
        f = s.navigate_to("home")
        fld = s.find(f, "field")
        s.click(fld, 1.2)
        f = s.capture("field-clicked")
        here, _ = s.where(f)
        checks.that(
            "clicking the search field does not restart setup",
            here.name == "home",
            f"landed on {here.name} - the field's hit box is routing to setup again",
        )

        qmp.type(args.query)
        time.sleep(0.8)
        f = s.capture("typed")
        fld = s.find(f, "field")
        typed = fld.label if fld else ""
        dot = s.find(f, "status_dot")
        dot_state = dot.label if dot else "unknown"
        print(f"  field reads back {typed!r}; nav dot {dot_state}")
        # Render the query into the field's own text box and compare coverage.
        # Correct text scores ~0.99 and a single missing character drops it
        # below 0.95, which is a sharper instrument than reading the field back
        # letter by letter and comparing strings.
        fit = (
            frame_score(f, fld, args.query, faces)
            if fld
            else 0.0
        )
        checks.that(
            "every keystroke reached the screen",
            fit >= 0.95,
            f"typed {args.query!r}, screen shows {typed!r} (coverage {fit:.3f})",
        )

        qmp.key("ret")
        time.sleep(9)
        f = s.capture("answer")
        here, _ = s.where(f)
        print(f"  Enter landed on {here}")
        checks.that(
            "Enter goes somewhere",
            here.name != "home",
            "still on home - Enter did nothing",
        )

        rows = s.find(f, "rows")
        titles = screen.row_titles(f, faces, rows)
        kinds = screen.row_kinds(f, faces, rows)
        openable = [t for t, (_, o) in zip(titles, kinds) if o]
        for t, (k, o) in zip(titles, kinds):
            print(f"    [{k or '-':<8}]{'*' if o else ' '} {t}")
        checks.that(
            "the answer screen offers a way back",
            s.find(f, "back") is not None,
            "no Back link on the answer screen",
        )

        # Bug class 7: the same document listed twice under two labels.
        dupes = duplicate_results(titles, kinds)
        checks.that("no result is listed twice", not dupes, "; ".join(dupes))

        # Bug class 5: a sentence naming a source the rows did not come from.
        offline_prose = [t for t in titles if "offline" in t.lower()]
        checks.that(
            "the report does not call the bridge offline while the guest shows it online",
            not (offline_prose and dot_state == "online"),
            f"nav dot says online, report says {offline_prose}",
        )

        # Bug class 3: prose that counts rows the screen did not draw.
        checks.that(
            "the bridge stayed up for the whole run",
            not witness.restarted(),
            "the bridge process was replaced mid-run - QEMU's COM2 chardev dies "
            "with it, so anything below this line is about a disconnected guest",
        )
        wire = witness.tail()
        sent = [ln for ln in wire.splitlines() if "agent.act" in ln and "goal=" in ln]
        answered = checks.that(
            "the bridge received the query during this run",
            bool(sent),
            f"nothing new in .bridge.log since the run started; guest nav dot "
            f"read {dot_state!r}",
        )
        if answered:
            got = sent[-1].split("goal=", 1)[1].strip()
            checks.that(
                "every keystroke reached the bridge",
                got.startswith(args.query),
                f"typed {args.query!r}, bridge received {got!r}",
            )
            claimed = None
            for line in reversed(wire.splitlines()):
                if "OK agent.act" in line and "n=" in line:
                    for tok in line.split():
                        if tok.startswith("n="):
                            claimed = int(tok[2:])
                    break
            checks.that(
                "the answer shows as many openable rows as the bridge returned",
                claimed is not None and claimed == len(openable),
                f"bridge returned n={claimed}, screen drew {len(openable)} "
                f"openable rows out of {len(rows)}",
            )
        else:
            checks.skip(
                "row count",
                f"bridge saw nothing this run; screen drew {len(rows)} rows",
            )

        log = serial.read_text(errors="replace") if serial.exists() else ""
        checks.that(
            "the kernel did not fault",
            "FAULT" not in log,
            next((ln for ln in log.splitlines() if "FAULT" in ln), ""),
        )
        checks.that(
            "the kernel did not panic",
            "PANIC" not in log,
            next((ln for ln in log.splitlines() if "PANIC" in ln), ""),
        )
        checks.that("a pointer came up", "usb-tablet ready" in log)

        rc = checks.report()
    except Stuck as e:
        print(f"\nNAVIGATION FAILED: {e}")
        rc = (checks.report() if checks.results else 0) or 1
    finally:
        qemu.terminate()
        try:
            qemu.wait(timeout=5)
        except subprocess.TimeoutExpired:
            qemu.kill()

    if s:
        print(f"\n{len(s.shots)} screenshots in {out}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
