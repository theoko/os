#!/usr/bin/env python3
"""Boot the ISO, drive it, and photograph the screen.

Every UI bug this project has shipped was found by a human looking at a
screenshot: the "Go Back" link drawn below the bottom edge, the same "bridge
is offline" sentence printed twice in two wordings, "5 matches" above three
rows. The unit tests passed through all of them, because none of those bugs
are visible from inside a function - they are visible from in front of the
screen.

This drives the real ISO in QEMU through the QMP socket: absolute mouse moves
and clicks against the usb-tablet, real keystrokes, and a screendump after
each step. It cannot judge whether a screen looks good. It can prove what is
actually on it, which is the part that kept being assumed.

    ./scripts/drive-ui.py                      # full journey
    ./scripts/drive-ui.py --out /tmp/shots     # where the PNGs land
"""

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Read from the first screendump rather than assumed. Guessing 1024x768 when
# Limine handed the kernel 1280x800 scaled every click to the wrong place, and
# the run looked like "the buttons do not work".
WIDTH, HEIGHT = 0, 0

# theme::ACCENT. The primary action is the only wide blob of it on screen.
ACCENT = (0x00, 0x71, 0xE3)

# Capability row geometry, matching setup.rs at this framebuffer size. Read
# straight off the zones the kernel registers, so a layout change that moves
# them shows up as a run that grants the wrong switch rather than silently
# clicking nothing.
CAP_ROW_X, CAP_ROW_TOP, CAP_ROW_PITCH, CAP_ROW_H = 640, 208, 54, 46

# Centre of ui::portal_rect at 1280x800, derived from search_rect: the pill is
# right-aligned to the field and sits 38px below it.
# Centre of ui::portal_rect at 1280x800: the pill is right-aligned to the
# 920px content column and sits 38px below the 132+52 search field.
PORTAL_CENTRE = (981, 236)

# Centre of the home search field at 1280x800 (920px column, y=132, h=52).
# Typing works on a freshly drawn home screen without clicking, but after
# navigating away and back the keystrokes went nowhere - so click it, which is
# what a person does anyway.
SEARCH_FIELD_CENTRE = (640, 158)

# The persistent nav "Back" link, top-left on every non-home screen.
NAV_BACK = (42, 23)

# theme::CARD_BORDER. Result rows are the only thing drawn at the exact left
# edge of the content column, which makes counting them a matter of counting
# runs of border pixels in one column rather than guessing at text.
CARD_BORDER = (0xE8, 0xE8, 0xED)

# searchui geometry: PAD_X 28, CONTENT_MAX 720, field at y=150 h=52, then a
# 26px gap and a 34px band for the agent's sentence before the first result.
SEARCH_PAD_X, SEARCH_CONTENT_MAX = 28, 720
SEARCH_RESULTS_TOP = 150 + 52 + 26 + 34

# Keystrokes QEMU knows by name; anything else has to be spelled out.
KEYMAP = {
    " ": "spc",
    ".": "dot",
    ",": "comma",
    "-": "minus",
    "/": "slash",
    "?": "shift-slash",
}


class Qmp:
    """Minimal QMP client. Enough to move a mouse and take a picture."""

    def __init__(self, path):
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
        self._read()  # greeting
        self.cmd("qmp_capabilities")

    def _read(self):
        while True:
            line = self.f.readline()
            if not line:
                raise RuntimeError("QMP closed")
            msg = json.loads(line)
            # Events arrive unsolicited; keep reading until a reply.
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
        self.cmd(
            "input-send-event",
            events=[
                {"type": "abs", "data": {"axis": "x", "value": x * 32767 // WIDTH}},
                {"type": "abs", "data": {"axis": "y", "value": y * 32767 // HEIGHT}},
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
        arrived as "i wanna wy paper". 25 keys a second is about 300 words a
        minute, so that says more about the harness than the kernel; typing at
        a speed a person could actually reach keeps the test about the product.
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


def read_ppm(path):
    """QEMU writes binary P6. Header is ASCII, then raw RGB triples."""
    data = path.read_bytes()
    fields, pos = [], 2  # skip "P6"
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
    pos += 1  # single whitespace after maxval
    w, h, _ = fields
    return w, h, data[pos : pos + w * h * 3]


def find_primary_button(path, tol=28, min_w=90, min_h=20):
    """Centre of the largest *solid* block of accent colour.

    Width alone is not enough: a selected row is outlined in accent and is
    520px across, three times wider than the Continue pill, so "widest run"
    reliably picked the wrong control. A filled pill is the only accent shape
    that is both wide and tall, so require both.
    """
    w, h, px = read_ppm(path)

    def accent_span(y):
        """First-to-last accent pixel, and how solidly it is filled.

        Contiguous runs do not work: the pill has the word "Continue" written
        through it in white, which splits its widest row into two halves that
        are each narrower than a selected row's outline. The span survives the
        text; the density check stops two unrelated marks being read as one
        wide control.
        """
        row = y * w * 3
        first = last = None
        count = 0
        for x in range(w):
            i = row + x * 3
            if all(abs(px[i + c] - ACCENT[c]) <= tol for c in range(3)):
                if first is None:
                    first = x
                last = x
                count += 1
        if first is None:
            return (0, 0, 0)
        span = last - first + 1
        # An outline is a thin frame: mostly background between its edges.
        return (span, first, last + 1) if count * 100 >= span * 55 else (0, 0, 0)

    runs = [accent_span(y) for y in range(h)]

    # Walk down, grouping rows whose wide runs overlap horizontally.
    best_blob, y = None, 0
    while y < h:
        if runs[y][0] < min_w:
            y += 1
            continue
        top, x0, x1 = y, runs[y][1], runs[y][2]
        while y < h and runs[y][0] >= min_w and runs[y][2] > x0 and runs[y][1] < x1:
            x0, x1 = max(x0, runs[y][1]), min(x1, runs[y][2])
            y += 1
        height = y - top
        if height >= min_h:
            area = height * (x1 - x0)
            if best_blob is None or area > best_blob[0]:
                best_blob = (area, (x0 + x1) // 2, top + height // 2)

    return None if best_blob is None else (best_blob[1], best_blob[2])


def count_result_rows(path):
    """How many result cards are actually drawn.

    The agent's sentence counts the rows it returned; the screen shows what it
    could fit. Those disagreed in shipped code - "5 matches for nvda." printed
    above three cards - and no unit test could see it, because the defect only
    exists once both halves are on screen together. Counting the cards is the
    only way to check the sentence against reality.
    """
    w, h, px = read_ppm(path)
    cw = min(w - SEARCH_PAD_X * 2, SEARCH_CONTENT_MAX)
    x = (w - cw) // 2  # left edge of the column: where card borders live

    # Start below the input field. The field is a rounded rect on the same
    # column, so counting from the top made it look like an extra result.
    rows, inside = 0, False
    for y in range(SEARCH_RESULTS_TOP, h):
        i = (y * w + x) * 3
        border = all(abs(px[i + c] - CARD_BORDER[c]) <= 12 for c in range(3))
        if border and not inside:
            rows += 1
            inside = True
        elif not border:
            inside = False
    return rows


class Checks:
    """Collected assertions. Reported together so one failure does not hide
    the rest, and the run exits non-zero if any failed."""

    def __init__(self):
        self.results = []

    def that(self, name, ok, detail=""):
        self.results.append((bool(ok), name, detail))
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f" - {detail}" if detail else ""))
        return ok

    def report(self):
        failed = [r for r in self.results if not r[0]]
        print(f"\n{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        for _, name, detail in failed:
            print(f"  FAILED: {name} - {detail}")
        return 1 if failed else 0


def to_png(ppm, png):
    """QEMU writes PPM; nothing else here reads PPM."""
    subprocess.run(
        ["sips", "-s", "format", "png", str(ppm), "--out", str(png)],
        check=True,
        capture_output=True,
    )
    ppm.unlink(missing_ok=True)


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
    ap.add_argument("--portal", default="1", help="click the portal pill; empty to skip")
    ap.add_argument(
        "--grant",
        default="2,5",
        help="capability rows to switch on at setup (0-indexed, screen order)",
    )
    args = ap.parse_args()

    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    # Start (or restart) the bridge before booting. Twice now this harness has
    # driven a guest against a bridge process older than its own binary and
    # reported the old behaviour as current - which is exactly the failure the
    # harness exists to catch.
    if args.bridge:
        subprocess.run(
            [str(ROOT / "scripts" / "ensure-bridge.sh")],
            cwd=ROOT,
            check=True,
            capture_output=True,
        )
        time.sleep(1.5)

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
            # Same topology make-utm.sh builds: one UHCI controller, one
            # device on it. Driving anything else would not be driving what
            # ships.
            "-device", "piix3-usb-uhci,id=uhci0",
            "-device", "usb-tablet,bus=uhci0.0",
            "-qmp", f"unix:{qmp_path},server=on,wait=off",
            "-no-reboot",
        ]
        + (
            # COM2 to the host bridge, exactly as make-utm.sh wires it.
            # Without this the answer screen only ever exercised the offline
            # fallback, so the agent path went unverified in the one place it
            # is actually visible.
            ["-serial", f"tcp:{args.bridge},server=off"] if args.bridge else []
        ),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
    )

    shots = []
    rc = 0

    def snap(name, keep_ppm=False):
        global WIDTH, HEIGHT
        ppm = out / f"{name}.ppm"
        png = out / f"{name}.png"
        qmp.shot(ppm)
        if not WIDTH:
            WIDTH, HEIGHT, _ = read_ppm(ppm)
            print(f"  framebuffer: {WIDTH}x{HEIGHT}")
        target = find_primary_button(ppm)
        if keep_ppm:
            subprocess.run(
                ["sips", "-s", "format", "png", str(ppm), "--out", str(png)],
                check=True, capture_output=True,
            )
        else:
            to_png(ppm, png)
        shots.append(png)
        print(f"  shot: {png}" + (f"  primary at {target}" if target else "  (no primary action)"))
        return target

    try:
        qmp = Qmp(str(qmp_path))
        print("booting")
        time.sleep(14)
        target = snap("01-welcome")

        # Walk the setup journey by finding the primary action each time
        # rather than assuming where it sits.
        for name in ["02-region", "03-bridge", "04-capabilities", "05-skills", "06-done", "07-home"]:
            if target is None:
                print(f"  no primary action before {name}; using Back to reach home")
                qmp.click(*NAV_BACK)
                time.sleep(2.0)
                snap(f"{name}-viaback")
                break
            qmp.click(*target)
            time.sleep(2.5)
            target = snap(name)

            # Grant capabilities on the way past. A journey that only ever
            # accepts the defaults never exercises a granted source, which is
            # most of what the agent does.
            if name == "04-capabilities" and args.grant:
                for i in [int(v) for v in args.grant.split(",") if v.strip()]:
                    x, y = CAP_ROW_X, CAP_ROW_TOP + i * CAP_ROW_PITCH + CAP_ROW_H // 2
                    print(f"  granting capability row {i} at ({x}, {y})")
                    qmp.click(x, y)
                    time.sleep(0.8)
                target = snap("04b-granted")

        # Type straight into the home field, no click needed.
        # Click the portal affordance before typing. It is drawn next to the
        # search field; a control that is drawn but does nothing is a bug this
        # project has shipped twice, so prove it routes somewhere.
        if args.portal:
            px, py = PORTAL_CENTRE
            print(f"  clicking the portal pill at ({px}, {py})")
            qmp.click(px, py)
            time.sleep(2.5)
            snap("07b-portal")
            # Esc does not return home from every screen; the nav Back link
            # does. Relying on Esc left the guest on the portal screen, so the
            # query typed next went nowhere and the bridge never saw it.
            qmp.click(*NAV_BACK)
            time.sleep(2.0)
            snap("07c-back")

        qmp.click(*SEARCH_FIELD_CENTRE)
        time.sleep(1.0)
        qmp.type(args.query)
        time.sleep(0.6)
        snap("08-typed")
        qmp.key("ret")
        time.sleep(8)
        snap("09-answer", keep_ppm=True)

        # ---- assertions -------------------------------------------------
        print("\nchecks:")
        checks = Checks()
        log = serial.read_text(errors="replace") if serial.exists() else ""

        checks.that("the kernel did not fault", "FAULT" not in log,
                    next((l for l in log.splitlines() if "FAULT" in l), ""))
        checks.that("the kernel did not panic", "PANIC" not in log,
                    next((l for l in log.splitlines() if "PANIC" in l), ""))
        checks.that("a pointer came up", "usb-tablet ready" in log)
        checks.that("the setup journey ran", "setup welcome" in log)

        # Keystrokes must arrive intact. Typing once produced "i wanna wy
        # paper" from "i wanna work on my paper", and nothing on screen or in
        # the kernel log revealed it - only the goal the bridge received did.
        bridge_log = ROOT / ".bridge.log"
        wire = bridge_log.read_text(errors="replace") if bridge_log.exists() else ""
        sent = [l for l in wire.splitlines() if "agent.act" in l and "goal=" in l]

        # Establish this first. Without it, a bridge that never saw the query
        # makes the next two checks fail with "received ''" and "n=None",
        # which reads like a UI bug and is not one.
        answered = checks.that(
            "the bridge received the query at all",
            bool(sent),
            "no agent.act reached the bridge - COM2 down, or it restarted mid-run",
        )
        got = sent[-1].split("goal=", 1)[1].strip() if sent else ""
        if answered:
            checks.that(
                "every keystroke reached the bridge",
                got.startswith(args.query),
                f"typed {args.query!r}, bridge received {got!r}",
            )

        # The sentence counts rows; the screen shows cards. They must agree.
        claimed = None
        for line in reversed(wire.splitlines()):
            if "OK agent.act" in line and "n=" in line:
                for f in line.split():
                    if f.startswith("n="):
                        claimed = int(f[2:])
                break
        drawn = count_result_rows(out / "09-answer.ppm")
        if answered:
            checks.that(
                "the answer shows as many rows as it claims",
                claimed is not None and claimed == drawn,
                f"bridge claimed n={claimed}, screen drew {drawn} cards",
            )
        else:
            print(f"  SKIP  row count (bridge silent; screen drew {drawn} cards)")

        rc = checks.report()

        print("\nserial:")
        if serial.exists():
            for line in serial.read_text(errors="replace").splitlines():
                if any(k in line for k in ("FAULT", "PANIC", "input:", "ui:", "mouse:")):
                    print("  " + line)
    finally:
        qemu.terminate()
        try:
            qemu.wait(timeout=5)
        except subprocess.TimeoutExpired:
            qemu.kill()

    print(f"\n{len(shots)} screenshots in {out}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
