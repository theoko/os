#!/usr/bin/env python3
"""End-to-end drive of the ARM64 guest in VirtualBox.

This is the guest the owner actually watches, and the one the duplicate-row bug
was seen on. It is also the blindest target in the project:

  * no QMP - VirtualBox has no equivalent, so there is no "screendump on
    demand over a socket, mouse events over the same socket" channel;
  * no readable serial from the kernel - the VM is wired --uart1 0x3f8
    --uart-mode1 file, and that file only ever contains UEFI's own output.
    Measured, not assumed: /tmp/teddyos-arm64.log after a full boot is 157
    bytes of firmware banner and ANSI clears, and nothing the kernel printed.
    So none of drive-ui.py's serial assertions ("the kernel did not fault",
    "a pointer came up", "setup welcome") port over.

    The port still has to *exist*, though, and that is not cosmetic - see
    Vbox.create();
  * no mouse, at all - `VBoxManage controlvm` offers keyboardputscancode,
    keyboardputstring and keyboardputfile, and nothing that moves a pointer.
    There is no VBoxManage verb for pointer input. Every x86 assertion in
    drive-ui.py that depends on clicking is simply not reachable here.

What is left is exactly two things, both verified by running them on this
machine rather than read out of a manual:

    VBoxManage controlvm <vm> screenshotpng <file>   - the guest framebuffer
    VBoxManage controlvm <vm> keyboardputstring/putscancode - keys in

So the whole harness is built on those two. Screenshots come back as PNG at the
guest's own resolution, and macOS's Vision framework turns them back into text
(scripts/e2e/ocr.swift), which is what lets this derive where it is rather than
counting steps. The journey is not scripted: each turn it photographs the
screen, works out which screen that is from the words on it, picks the next key
from that, and photographs the result to confirm the screen changed the way it
was supposed to.

Because there is no pointer, the journey is keyboard-only. That is not a
compromise the harness invented - setup.rs::key exists precisely so first boot
can be completed with Enter, and Escape leaves the Brief for Home. Driving the
paths a keyboard-only user has is a fair test; it is just a smaller one than
the x86 driver gets.

    ./scripts/e2e/arm64.py                       # boot, drive, assert
    ./scripts/e2e/arm64.py --query "nvda"        # what to type on home
    ./scripts/e2e/arm64.py --no-boot             # drive whatever is running
    ./scripts/e2e/arm64.py --selfcheck           # prove the checks can fail
    ./scripts/e2e/arm64.py --selftest            # re-check the saved frames

There is one teddyOS-arm64 on the machine and more than one session in this
repo. If someone else is driving it, give this run a VM of its own:

    ./scripts/e2e/arm64.py --vm teddyOS-arm64-e2e --create --frontend headless

A run that gets power-cycled underneath it aborts with status 2 and says so,
rather than reporting the reboot as a dead control.

Exit status is non-zero if any check failed.
"""

import argparse
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = Path(__file__).resolve().parent

VM = os.environ.get("VBOX_VM_NAME", "teddyOS-arm64")

# A framebuffer smaller than this is not a screen anyone can use, and it is the
# symptom of Limine silently falling back when the firmware does not offer the
# mode that was asked for. Deliberately well under 1280x800 so it only trips on
# a real fallback, not on a different-but-sane choice.
MIN_W, MIN_H = 1024, 640

# How much of its own framebuffer the kernel has to paint. The compositor fills
# the whole surface with theme::BG before drawing anything, so the honest
# expectation is "all of it"; the slack is only for a stray uninitialised edge.
MIN_PAINTED = 0.99

BLACK = b"\x00\x00\x00"

# AT set-1 make/break pairs. VirtualBox translates these into the USB HID
# usages the guest's keyboard driver reads (hid.rs: 0x28 Enter, 0x29 Escape,
# 0x2B Tab), which is why scancodes work at all against a --keyboard usb VM.
SCANCODES = {
    "esc": ["01", "81"],
    "enter": ["1c", "9c"],
    "tab": ["0f", "8f"],
    "backspace": ["0e", "8e"],
    "up": ["e0", "48", "e0", "c8"],
    "down": ["e0", "50", "e0", "d0"],
}


# --------------------------------------------------------------------------
# VBoxManage
# --------------------------------------------------------------------------


class Vbox:
    def __init__(self, name):
        self.name = name
        if not shutil.which("VBoxManage"):
            raise SystemExit("VBoxManage not on PATH - install VirtualBox 7.2+")

    def manage(self, *args, check=True):
        p = subprocess.run(
            ["VBoxManage", *args], capture_output=True, text=True
        )
        if check and p.returncode != 0:
            raise RuntimeError(
                f"VBoxManage {' '.join(args)} failed ({p.returncode}): "
                f"{p.stderr.strip() or p.stdout.strip()}"
            )
        return p.stdout

    def info(self):
        out = self.manage("showvminfo", self.name, "--machinereadable", check=False)
        d = {}
        for line in out.splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                d[k.strip('"')] = v.strip().strip('"')
        return d

    def state(self):
        return self.info().get("VMState", "absent")

    def session_key(self):
        """Identity of the *current* boot of this VM.

        There is exactly one teddyOS-arm64 on this machine and more than one
        session works on this repo at a time. While the first version of this
        script was running, another process was sending `keyboardputscancode
        1c 9c` to the same VM every three seconds and power-cycling it between
        attempts (ps showed both). The run reported "Enter on Welcome changed
        nothing - the control is drawn but not wired", which is a product bug
        that does not exist: the guest had been rebooted underneath it and was
        showing a *fresh* Welcome each time it was photographed.

        That is precisely the failure this suite exists to stop reporting. So
        the boot gets an identity, and any check that spans time re-reads it.
        """
        i = self.info()
        return (i.get("VMState"), i.get("VMStateChangeTime"), i.get("SessionPID"))

    def create(self):
        """Make a VM matching scripts/make-virtualbox-arm64.sh.

        Same settings, because a harness that drives a differently-configured
        machine is not driving what ships. OHCI in particular: VirtualBox ARM
        exposes its built-in keyboard through OHCI, and EHCI/xHCI move it
        behind controllers this kernel does not drive.

        The serial port is load-bearing and this was found the hard way. A VM
        created with every other setting identical but `uart1=off` boots, draws
        a perfect Home screen, and then accepts no input at all - no key ever
        reaches the guest, and its VBox.log never contains the "OHCI: USB
        Operational" line the working VM logs at t+6.7s. Turning uart1 on and
        changing nothing else took the same VM from "0 of 5 keystrokes arrive"
        to the full setup journey plus a typed query. So the guest apparently
        does not survive its own boot-time serial writes when there is no UART
        behind them, and it dies before it initialises USB.

        That is a real fragility worth knowing about: it means the ARM64 ISO is
        only driveable on a VM that happens to have a serial port configured,
        and make-virtualbox-arm64.sh sets one for logging, so nobody had
        noticed the dependency.
        """
        # `--os-type` (as make-virtualbox-arm64.sh spells it) is rejected by
        # VBoxManage 7.2.14: the option is `--ostype`, and an ARM guest also
        # needs --platform-architecture arm or createvm builds an x86 machine
        # that will not take --chipset armv8virtual.
        self.manage("createvm", "--name", self.name,
                    "--platform-architecture", "arm",
                    "--ostype", "Other_arm64", "--register")
        self.manage(
            "modifyvm", self.name,
            "--chipset", "armv8virtual", "--firmware", "efi",
            "--memory", "2048", "--cpus", "4",
            "--graphicscontroller", "qemuramfb", "--vram", "128",
            "--boot1", "dvd", "--boot2", "none", "--boot3", "none", "--boot4", "none",
            "--usb-ohci", "on", "--usb-ehci", "off", "--usb-xhci", "off",
            "--mouse", "usbtablet", "--keyboard", "usb",
            "--uart1", "0x3f8", "4",
            "--uart-mode1", "file", f"/tmp/{self.name}-uart.log",
            "--audio-enabled", "off", "--nic1", "nat",
        )
        self.manage("storagectl", self.name, "--name", "SATA",
                    "--add", "sata", "--controller", "IntelAhci")
        self.manage("setextradata", self.name,
                    "VBoxInternal2/EfiGraphicsResolution", "1280x800")

    def log_path(self):
        folder = self.info().get("LogFldr")
        return Path(folder) / "VBox.log" if folder else None

    def poweroff(self):
        if self.state() not in ("poweroff", "aborted", "absent"):
            self.manage("controlvm", self.name, "poweroff", check=False)
        for _ in range(100):
            if self.state() in ("poweroff", "aborted", "absent"):
                return
            time.sleep(0.2)
        raise RuntimeError(f"{self.name} would not power off")

    def attach_iso(self, iso):
        # Detaching first matters: VirtualBox otherwise keeps serving the prior
        # contents of a rebuilt ISO out of its already-open medium.
        # Eject first: VirtualBox otherwise keeps serving the previous contents
        # of a rebuilt ISO out of its already-open medium, so a run can drive
        # the build before the one under test.
        #
        # `--medium emptydrive` and not `--medium none`. `none` deletes the
        # drive slot itself, and the re-attach then fails with
        # VBOX_E_OBJECT_NOT_FOUND ("No drive attached to device slot 0") -
        # which is how this VM lost its DVD drive on the first run of this
        # script. `emptydrive` ejects the disc and leaves the drive in place,
        # and it also recreates the slot if something already removed it.
        self.manage(
            "storageattach", self.name, "--storagectl", "SATA",
            "--port", "0", "--device", "0", "--type", "dvddrive",
            "--medium", "emptydrive",
        )
        self.manage(
            "storageattach", self.name, "--storagectl", "SATA",
            "--port", "0", "--device", "0", "--type", "dvddrive",
            "--medium", str(iso),
        )

    def start(self, frontend="gui"):
        self.manage("startvm", self.name, "--type", frontend)

    def shot(self, path):
        self.manage("controlvm", self.name, "screenshotpng", str(path))
        return Path(path)

    def key(self, name):
        self.manage("controlvm", self.name, "keyboardputscancode", *SCANCODES[name])

    def put(self, text):
        # keyboardputstring takes the string as an argv word, so a leading '-'
        # would be read as an option. Nothing here needs one; refuse rather
        # than silently type something else.
        if text.startswith("-"):
            raise ValueError("keyboardputstring cannot send a leading '-'")
        self.manage("controlvm", self.name, "keyboardputstring", text)

    def type(self, text, chunk=8, delay=0.35):
        """Type in small chunks.

        drive-ui.py found that hammering keys straight through dropped them -
        "i wanna work on my paper" arrived as "i wanna wy paper". That was QEMU,
        but the guest side of it (a single-slot USB interrupt queue polled once
        a frame) is the same code here, so the same caution applies. The check
        that the field holds what was typed is what actually proves it.
        """
        for i in range(0, len(text), chunk):
            self.put(text[i:i + chunk])
            time.sleep(delay)


# --------------------------------------------------------------------------
# PNG
# --------------------------------------------------------------------------


def read_png(path):
    """Decode a VirtualBox screenshotpng into (w, h, RGB bytes).

    Written out rather than pulled from Pillow because this repo has no Python
    dependencies and adding one to a test harness is how harnesses stop being
    run.
    """
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path} is not a PNG")
    pos, idat, plte = 8, bytearray(), None
    w = h = depth = ctype = None
    while pos < len(data):
        (ln,) = struct.unpack(">I", data[pos:pos + 4])
        typ = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + ln]
        pos += 12 + ln
        if typ == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if interlace:
                raise ValueError("interlaced PNG unsupported")
            if depth != 8:
                raise ValueError(f"bit depth {depth} unsupported")
        elif typ == b"PLTE":
            plte = body
        elif typ == b"IDAT":
            idat += body
        elif typ == b"IEND":
            break
    raw = zlib.decompress(bytes(idat))
    bpp = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    stride = w * bpp
    out = bytearray(h * stride)
    prev = bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]
        p += 1
        line = bytearray(raw[p:p + stride])
        p += stride
        if f == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif f == 3:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif f == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                c = prev[i - bpp] if i >= bpp else 0
                b = prev[i]
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        out[y * stride:(y + 1) * stride] = line
        prev = line
    if ctype == 2:
        rgb = bytes(out)
    else:
        rgb = bytearray(w * h * 3)
        for i in range(w * h):
            if ctype == 6:
                rgb[i * 3:i * 3 + 3] = out[i * 4:i * 4 + 3]
            elif ctype == 0:
                rgb[i * 3:i * 3 + 3] = bytes((out[i],)) * 3
            elif ctype == 4:
                rgb[i * 3:i * 3 + 3] = bytes((out[i * 2],)) * 3
            else:
                j = out[i] * 3
                rgb[i * 3:i * 3 + 3] = plte[j:j + 3]
        rgb = bytes(rgb)
    return w, h, rgb


PAGE = b"\xff\xff\xff"  # theme::BG


def ink_stats(w, h, px):
    """How much of the screen has anything drawn on it.

    Separate from paint_stats on purpose. "Painted" answers "did the kernel own
    this framebuffer", and is 100% on every healthy screen because the
    compositor clears to theme::BG first. "Ink" answers the question people
    actually ask when they look at the window - how much of it has content -
    and the two are wildly different numbers:

        setup Welcome @1280x800   ink 8803px = 0.86%, bbox 180x161
        setup Welcome @1920x1080  ink 8803px = 0.42%, bbox 180x161
        Home          @1280x800   ink 13743px = 1.34%, bbox 1280x754

    Identical ink at both resolutions is the whole story: the UI is laid out in
    fixed pixels and centred, so enlarging the framebuffer does not enlarge
    anything drawn on it - it just adds page around it.
    """
    minx, miny, maxx, maxy, n = w, h, -1, -1, 0
    for y in range(h):
        row = y * w * 3
        for x in range(w):
            i = row + x * 3
            if px[i:i + 3] != PAGE:
                n += 1
                if x < minx:
                    minx = x
                if x > maxx:
                    maxx = x
                if y < miny:
                    miny = y
                if y > maxy:
                    maxy = y
    if maxx < 0:
        return {"pixels": 0, "fraction": 0.0, "bbox_w": 0, "bbox_h": 0}
    return {
        "pixels": n,
        "fraction": n / float(w * h),
        "bbox_w": maxx - minx + 1,
        "bbox_h": maxy - miny + 1,
    }


def iso_resolution(iso):
    """The `resolution:` Limine was asked for, read out of the ISO itself.

    This is the only thing that actually moves the guest's mode, and it was
    measured rather than assumed:

      * VBoxInternal2/EfiGraphicsResolution does NOT control it. Set to
        800x600 with everything else unchanged, the guest still came up
        1280x800. It only changes the mode the firmware starts in - the log
        shows 800x600 then Limine switching away from it.
      * os-arm64.iso as it ships contains no `resolution:` line at all (the
        block in limine.conf is entirely comments), so Limine picks the mode,
        and it picked 1280x800.
      * Rebuilding with `make arm64-iso RESOLUTION=1920x1080x32` appends a real
        `resolution:` line, and the guest then came up at exactly 1920x1080.

    Reading it from the ISO rather than from limine.conf in the working tree
    matters: the tree can be edited after the ISO was built, and this has to
    describe the image actually being booted.
    """
    try:
        data = Path(iso).read_bytes()
    except OSError:
        return None
    m = re.search(rb"\n\s*resolution:\s*(\d+)x(\d+)", data)
    return (int(m.group(1)), int(m.group(2))) if m else None


def paint_stats(w, h, px):
    """Painted fraction and bounding box of everything that is not black.

    Black is the right sentinel: VirtualBox hands the device a zeroed surface,
    and every kernel screen opens with fill(theme::BG) which is near-white. A
    guest that came up in a mode it cannot address, or that drew into a corner
    of a larger surface, leaves the rest at zero and that shows up here.
    """
    minx, miny, maxx, maxy, lit = w, h, -1, -1, 0
    for y in range(h):
        row = y * w * 3
        for x in range(w):
            i = row + x * 3
            if px[i:i + 3] != BLACK:
                lit += 1
                if x < minx:
                    minx = x
                if x > maxx:
                    maxx = x
                if y < miny:
                    miny = y
                if y > maxy:
                    maxy = y
    if maxx < 0:
        return {"fraction": 0.0, "bbox": None, "bbox_w": 0, "bbox_h": 0}
    return {
        "fraction": lit / float(w * h),
        "bbox": (minx, miny, maxx, maxy),
        "bbox_w": maxx - minx + 1,
        "bbox_h": maxy - miny + 1,
    }


def distinct_colours(w, h, px, step=2):
    seen = set()
    for y in range(0, h, step):
        row = y * w * 3
        for x in range(0, w, step):
            i = row + x * 3
            seen.add(px[i:i + 3])
    return len(seen)


# --------------------------------------------------------------------------
# OCR
# --------------------------------------------------------------------------


class Ocr:
    """macOS Vision, via a tiny Swift tool compiled once and cached."""

    def __init__(self):
        self.tool = None
        self.why = ""
        src = HERE / "ocr.swift"
        if not shutil.which("swiftc"):
            self.why = "swiftc not found (install Xcode command line tools)"
            return
        if not src.exists():
            self.why = f"missing {src}"
            return
        cache = Path(tempfile.gettempdir()) / f"os-e2e-ocr-{int(src.stat().st_mtime)}"
        cache.mkdir(parents=True, exist_ok=True)
        tool = cache / "ocrtool"
        if not tool.exists():
            p = subprocess.run(
                ["swiftc", "-O", "-o", str(tool), str(src)],
                capture_output=True, text=True,
            )
            if p.returncode != 0:
                self.why = f"swiftc failed: {p.stderr.strip()[:400]}"
                return
        self.tool = tool

    @property
    def available(self):
        return self.tool is not None

    def read(self, path):
        if not self.available:
            return None
        p = subprocess.run(
            [str(self.tool), str(path)], capture_output=True, text=True
        )
        if p.returncode != 0:
            return None
        return json.loads(p.stdout)["lines"]


# --------------------------------------------------------------------------
# a captured frame
# --------------------------------------------------------------------------


class Frame:
    def __init__(self, path, ocr=None):
        self.path = Path(path)
        self.w, self.h, self.px = read_png(self.path)
        self.paint = paint_stats(self.w, self.h, self.px)
        self.ink = ink_stats(self.w, self.h, self.px)
        self.colours = distinct_colours(self.w, self.h, self.px)
        self.lines = ocr.read(self.path) if ocr else None
        self.texts = [l["text"] for l in self.lines] if self.lines else []
        self.blob = " │ ".join(self.texts).lower()
        self.screen = classify(self)

    def has(self, *needles):
        return all(n.lower() in self.blob for n in needles)

    def describe(self):
        return (
            f"{self.w}x{self.h} painted={self.paint['fraction'] * 100:.2f}% "
            f"ink={self.ink['fraction'] * 100:.2f}%/{self.ink['bbox_w']}x"
            f"{self.ink['bbox_h']} colours={self.colours} screen={self.screen}"
        )

    def dump(self, limit=40):
        if self.lines is None:
            return "    (no OCR)"
        return "\n".join(
            f"    y={l['y']:4d} x={l['x']:4d}  {l['text']}"
            for l in self.lines[:limit]
        )


# Ordered most-specific-first. Anchors are the strings the kernel actually
# draws (setup.rs / screens.rs), so a redesign that renames a screen surfaces
# as "unknown" with the OCR printed underneath - which is a harness failure the
# reader can see, not a product failure the harness invented.
SIGNATURES = [
    ("setup/welcome", ["hello", "continue"], ["how should it feel"]),
    ("setup/experience", ["how should it feel"], []),
    ("setup/region", ["select your region"], []),
    ("setup/bridge", ["connect the bridge"], []),
    # Same step in a standalone image, where there is no host to connect to.
    # Without this the screen falls through to the catch-all "search" signature
    # (any screen with a Back button), which sends the wrong key and loops the
    # journey instead of reporting an unknown screen.
    ("setup/bridge", ["runs on its own"], []),
    ("setup/capabilities", ["capabilities", "continue"], ["default skills"]),
    ("setup/skills", ["default skills"], []),
    ("setup/done", ["all set"], []),
    ("brief", ["brief", "back"], ["last brief"]),
    ("status", ["status", "back"], []),
    ("search", ["back"], []),
    # Home is the only screen with all three capability cards and no Back.
    ("home", ["search", "capabilities", "skills"], ["back", "continue"]),
]


def classify(frame):
    if frame.lines is None:
        return "unreadable"
    if not frame.texts:
        return "blank"
    for name, need, forbid in SIGNATURES:
        if all(n in frame.blob for n in need) and not any(f in frame.blob for f in forbid):
            return name
    return "unknown"


# --------------------------------------------------------------------------
# checks
# --------------------------------------------------------------------------


class Checks:
    def __init__(self):
        self.results = []

    def that(self, name, ok, detail=""):
        # Detail is the failure explanation, so it only prints on failure.
        # Printing it under PASS produced lines like "PASS ... only 100.00% of
        # the framebuffer is non-black", which reads as a fault report.
        self.results.append((bool(ok), name, detail))
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f" - {detail}" if not ok and detail else ""))
        return bool(ok)

    def skip(self, name, why):
        self.results.append((None, name, why))
        print(f"  SKIP  {name} - {why}")

    def report(self):
        failed = [r for r in self.results if r[0] is False]
        passed = [r for r in self.results if r[0] is True]
        skipped = [r for r in self.results if r[0] is None]
        print(
            f"\n{len(passed)}/{len(passed) + len(failed)} checks passed"
            + (f", {len(skipped)} skipped" if skipped else "")
        )
        for _, name, detail in failed:
            print(f"  FAILED: {name} - {detail}")
        return 1 if failed else 0


def guest_mode_from_log(log):
    """The last mode VirtualBox's Display accepted from the guest.

    This is the number to compare a screenshot against: it is what the host
    believes the guest asked for, independent of what the guest then drew.
    """
    if not log or not Path(log).exists():
        return None
    last = None
    pat = re.compile(r"i_handleDisplayResize: uScreenId=0 .*? w=(\d+) h=(\d+) bpp=(\d+)")
    for line in Path(log).read_text(errors="replace").splitlines():
        m = pat.search(line)
        if m:
            last = (int(m.group(1)), int(m.group(2)), int(m.group(3)))
    return last


def check_rendering(checks, f, configured=None, mode=None):
    """Is a real UI on screen, and does it own the whole framebuffer?

    Three separate claims, kept separate so a failure says which one broke:
      - the mode is usable at all (not an 800x600 firmware fallback);
      - the screenshot is the size the host thinks the guest is running;
      - the kernel painted the surface it was handed, all of it.
    """
    checks.that(
        "the guest chose a usable mode",
        f.w >= MIN_W and f.h >= MIN_H,
        f"framebuffer is {f.w}x{f.h}, floor is {MIN_W}x{MIN_H}",
    )
    if configured:
        cw, ch = configured
        checks.that(
            "Limine honoured the ISO's resolution instead of falling back",
            (f.w, f.h) == (cw, ch),
            f"the ISO asks for {cw}x{ch}, the guest came up {f.w}x{f.h}. "
            f"Limine falls back silently when the firmware does not offer the "
            f"mode - pick one it does",
        )
    else:
        checks.skip(
            "Limine honoured the ISO's resolution",
            f"this ISO pins no resolution, so Limine chose {f.w}x{f.h} itself. "
            f"Set one with: make arm64-iso RESOLUTION={f.w}x{f.h}x32",
        )
    if mode:
        checks.that(
            "the screenshot is the size VirtualBox reports for the guest",
            (f.w, f.h) == (mode[0], mode[1]),
            f"Display reports {mode[0]}x{mode[1]}, screenshot is {f.w}x{f.h}",
        )
    else:
        checks.skip(
            "the screenshot matches VirtualBox's reported display",
            "no i_handleDisplayResize line in VBox.log",
        )
    checks.that(
        "the kernel painted its whole framebuffer",
        f.paint["fraction"] >= MIN_PAINTED,
        f"only {f.paint['fraction'] * 100:.2f}% of {f.w}x{f.h} is non-black "
        f"(bbox {f.paint['bbox']})",
    )
    checks.that(
        "the drawn region spans the framebuffer",
        f.paint["bbox_w"] >= f.w * 0.98 and f.paint["bbox_h"] >= f.h * 0.98,
        f"drawn bbox is {f.paint['bbox_w']}x{f.paint['bbox_h']} inside {f.w}x{f.h}",
    )
    # Firmware leaves a two-tone console; a compositor leaves gradients, rules,
    # accents and antialiased glyphs. This is the cheap half of "is it the OS".
    checks.that(
        "the screen is a composited UI, not a firmware console",
        f.colours >= 40,
        f"only {f.colours} distinct colours - looks like text mode or a splash",
    )
    if f.lines is None:
        checks.skip("the OS UI is on screen", "OCR unavailable")
    else:
        checks.that(
            "the OS UI is on screen, not firmware",
            f.screen not in ("blank", "unknown", "unreadable"),
            f"could not identify the screen; OCR said: {f.blob[:200]!r}",
        )


# Rows that name a *result* - a document, a message, an item the agent found.
# Two of these carrying the same title is the duplicate bug.
RESULT_TAGS = ("doc", "hit", "urgent", "reply", "need", "event", "fyi", "body")

# Rows that restate the request or explain the run. "Goal: paper" and
# "Query: paper" are *supposed* to say the same thing, so they must be kept out
# of the duplicate check. The first version of this check did not exclude them
# and only passed because Vision happened to misread one of the two as "papel".
# A check that survives on an OCR error is a check that will be deleted the
# first time OCR reads correctly.
META_TAGS = ("goal", "query", "next", "info", "plan", "blocked")

ROW_TAGS = RESULT_TAGS + META_TAGS


def brief_rows(frame):
    """The Report rows on a Brief, as (tag, text) pairs.

    Vision returns a row as one line ("Doc: os identity") or occasionally as
    the tag and the title separately when the gap is wide; both are folded here
    so the duplicate check is not at the mercy of OCR line-splitting.
    """
    rows = []
    pending_tag = None
    for l in frame.lines or []:
        t = l["text"].strip()
        low = t.lower().rstrip(":")
        if pending_tag is not None:
            rows.append((pending_tag, t))
            pending_tag = None
            continue
        m = re.match(r"^([A-Za-z]+)\s*[:│|]\s*(.+)$", t)
        if m and m.group(1).lower() in ROW_TAGS:
            rows.append((m.group(1).lower(), m.group(2).strip()))
        elif low in ROW_TAGS:
            pending_tag = low
    return rows


def normalise_title(s):
    return re.sub(r"[^a-z0-9]+", " ", s.lower()).strip()


def check_no_duplicate_rows(checks, frame):
    """The same document must not be listed twice under two labels.

    Shipped, and seen on this guest: a Brief carrying "Doc: os identity" *and*
    "Hit: os identity", and the same for "Agent skills". agent.rs reaches
    push_line("Doc", ...) and push_line("Hit", ...) from different branches
    over the same corpus, so a title that satisfies both arrives twice. No unit
    test sees it, because each branch is individually correct - the defect only
    exists once both are on the screen together.
    """
    if frame.lines is None:
        checks.skip("no result is listed twice", "OCR unavailable")
        return
    rows = [(t, x) for t, x in brief_rows(frame) if t in RESULT_TAGS]
    if not rows:
        checks.skip("no result is listed twice", "no result rows on this screen")
        return
    dupes = []
    for i in range(len(rows)):
        for j in range(i + 1, len(rows)):
            a, b = normalise_title(rows[i][1]), normalise_title(rows[j][1])
            if a and b and similar(a, b):
                dupes.append(f"{rows[i][0]}:{rows[i][1]!r} == {rows[j][0]}:{rows[j][1]!r}")
    checks.that("no result is listed twice", not dupes, "; ".join(dupes))


def edit_distance(a, b):
    if a == b:
        return 0
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def similar(a, b):
    """Same document, allowing for OCR slips.

    Vision read "paper" as "papel" on a real frame in this repo, so requiring
    an exact match would let a duplicate through on one bad glyph - and the
    whole point of this check is that two rows carry the *same* document.

    A ratio threshold was the first attempt and it was the wrong tool: at 0.92
    it missed "os identity" vs "os identlty" (ratio 0.909, a single
    substitution) while still being loose enough to be nervous about on long
    strings. An OCR slip is a small number of wrong characters regardless of
    length, so budget characters, not proportions: one per ten, minimum one.
    That catches the single-glyph misread and still keeps genuinely different
    neighbours apart - "thesis-draft.md" vs "thesis-final.md" is five edits on
    a budget of one.
    """
    if a == b:
        return True
    budget = max(1, min(len(a), len(b)) // 10)
    return edit_distance(a, b) <= budget


def check_openable_claim(checks, frame):
    """A Brief may not draw Doc rows and then deny they exist.

    Seen on this guest and reproduced on demand: three rows tagged "Doc",
    drawn in theme::ACCENT - the colour screens.rs reserves for rows you can
    act on - immediately above the sentence "No openable hits - refine the
    ask." Either the rows open something and the sentence is false, or they do
    not and they are painted to look like they do, which is the "drawn but not
    wired" class that has shipped twice.

    This is exactly the defect no unit test can see: agent.rs is individually
    right about the rows and individually right about the sentence, and they
    are only wrong together, on screen.
    """
    if frame.lines is None:
        checks.skip("the Brief agrees with itself about openable rows", "OCR unavailable")
        return
    rows = brief_rows(frame)
    docs = [x for t, x in rows if t == "doc"]
    denies_any = "no openable hits" in frame.blob
    if not docs and not denies_any:
        checks.skip("the Brief agrees with itself about openable rows",
                    "no Doc rows and no claim about them")
        return
    checks.that(
        "the Brief agrees with itself about openable rows",
        not (docs and denies_any),
        f"{len(docs)} Doc rows are drawn ({docs}) above the sentence "
        f"'No openable hits - refine the ask.'",
    )


def check_counts_agree(checks, frame):
    """A count in prose must match the rows underneath it.

    "5 matches for nvda." above three cards shipped. Any sentence on screen
    carrying a number of results is checked against the rows actually drawn.
    """
    if frame.lines is None:
        checks.skip("a claimed count matches the rows drawn", "OCR unavailable")
        return
    rows = brief_rows(frame)
    claims = []
    for l in frame.lines:
        m = re.search(r"\b(\d+)\s+(match(?:es)?|result(?:s)?|hit(?:s)?|doc(?:s)?)\b",
                      l["text"], re.I)
        if m:
            claims.append((int(m.group(1)), l["text"].strip()))
    if not claims:
        checks.skip("a claimed count matches the rows drawn", "no count claimed on screen")
        return
    for n, sentence in claims:
        checks.that(
            "a claimed count matches the rows drawn",
            n == len(rows),
            f"screen says {sentence!r} but {len(rows)} rows are drawn",
        )


def check_on_screen(checks, frame, margin=4):
    """Nothing the guest drew may sit on or past an edge.

    A control drawn off-screen or overlapping another is a class this project
    has shipped ("Go Back" pushed under the last row, below the bottom edge).
    Text that touches an edge is text that has probably been clipped.
    """
    if frame.lines is None:
        checks.skip("nothing is drawn off the edge", "OCR unavailable")
        return
    bad = [
        l["text"]
        for l in frame.lines
        if l["x"] < margin
        or l["y"] < margin
        or l["x"] + l["w"] > frame.w - margin
        or l["y"] + l["h"] > frame.h - margin
    ]
    checks.that(
        "nothing is drawn off the edge",
        not bad,
        f"touching an edge of {frame.w}x{frame.h}: {bad[:5]}",
    )


def check_no_repeated_sentence(checks, frame, min_len=18):
    """The same explanation must not be printed twice in two places.

    Shipped: "bridge is offline" rendered twice in two wordings. Exact repeats
    are the half that can be caught mechanically.
    """
    if frame.lines is None:
        checks.skip("no sentence is printed twice", "OCR unavailable")
        return
    seen = {}
    for l in frame.lines:
        t = normalise_title(l["text"])
        if len(t) >= min_len:
            seen[t] = seen.get(t, 0) + 1
    dupes = [t for t, n in seen.items() if n > 1]
    checks.that(
        "no sentence is printed twice",
        not dupes,
        f"repeated on screen: {dupes[:3]}",
    )


def check_field_holds(checks, frame, typed):
    """Every keystroke that was sent has to be on screen.

    Dropped keystrokes shipped once already ("i wanna work on my paper" became
    "i wanna wy paper") and nothing but reading the text back reveals them.
    """
    if frame.lines is None:
        checks.skip("every keystroke arrived", "OCR unavailable")
        return
    want = normalise_title(typed)
    got = [normalise_title(t) for t in frame.texts]
    hit = next((g for g in got if want in g), None)
    checks.that(
        "every keystroke arrived intact",
        hit is not None,
        f"typed {typed!r}; nothing on screen contains it. lines: {frame.texts[:8]}",
    )


# --------------------------------------------------------------------------
# the journey
# --------------------------------------------------------------------------


ADVANCE = {
    "setup/welcome": "enter",
    "setup/experience": "enter",
    "setup/region": "enter",
    "setup/bridge": "enter",
    "setup/capabilities": "enter",
    "setup/skills": "enter",
    "setup/done": "enter",
    # Setup hands off to the morning Brief, not to Home. Escape is the
    # keyboard's way back (main.rs: Brief + Escape -> Home).
    "brief": "esc",
    "status": "esc",
    "search": "esc",
}


class VmRestarted(RuntimeError):
    """Someone else power-cycled the VM in the middle of this run."""


def press_and_settle(vbox, key, snap, name, before, ocr_deadline=15, poll=1.5):
    """Send one key, then wait for the screen to actually change.

    A fixed sleep is what made the first run of this harness report "Enter is
    not wired on Welcome". It is wired; the guest had simply not finished
    enumerating its USB keyboard, and 2.5s later the screen was still Welcome.
    Pressing again would then have advanced two steps at once and hidden it the
    other way.

    So: press exactly once, then keep photographing until the screen differs or
    the deadline passes. The assertion survives - a control that really is not
    wired never changes the screen, and this still says so - but it is now the
    guest's own timing that decides when to look, not a number someone guessed.
    """
    vbox.key(key)
    start = time.time()
    frame = None
    while time.time() - start < ocr_deadline:
        time.sleep(poll)
        frame = snap(name)
        if frame.blob != before:
            return frame, True
    return frame, False


def check_first_screen_settles(checks, first, snap, wait=8.0):
    """The screen a fresh boot lands on must be the screen it means to show.

    This is the check that was missing when the harness reported "Enter on Home
    returns to the welcome screen" on arm64. It never did. The kernel painted a
    complete Home screen during boot and replaced it with setup Welcome about a
    second and a half later, and `wait_for_render` returned on whichever side of
    that flip it happened to photograph. Every keystroke this run then sent went
    to a screen that no longer existed, and the report blamed the input path.

    A guest that is finished booting owns its screen until something is typed at
    it, so: photograph, wait, photograph again, and require the answer not to
    have changed by itself. Nothing has been typed at this point, so a change
    here is the guest overpainting - never the harness.
    """
    time.sleep(wait)
    again = snap("01-settled")
    checks.that(
        "the screen the guest boots to is the screen it stays on",
        again.screen == first.screen,
        f"the guest landed on {first.screen!r} and turned into "
        f"{again.screen!r} {wait:.0f}s later without a key being pressed. "
        f"Whatever this run drives from here, it is not the screen it "
        f"photographed - so a keystroke assertion that fails below is about "
        f"the race, not the input path.",
    )
    return again


def drive_to_home(vbox, snap, checks, max_steps=12, settle=30):
    """Walk to Home by reading each screen, not by counting clicks.

    The x86 driver's "click Continue six times" broke every time the wizard got
    a step shorter. This asks the screen what it is each turn, and stops when
    the screen says Home. If a key does not change the screen, it says so
    rather than pressing on and blaming the next assertion.
    """
    seen = []
    frame = snap("10-first")
    for step in range(max_steps):
        seen.append(frame.screen)
        print(f"  at {frame.screen}")
        if frame.screen == "home":
            checks.that(
                "the setup journey reaches Home on the keyboard alone",
                True,
                f"path: {' -> '.join(seen)}",
            )
            return frame
        key = ADVANCE.get(frame.screen)
        if key is None:
            checks.that(
                "the setup journey reaches Home on the keyboard alone",
                False,
                f"stuck on {frame.screen!r} after {' -> '.join(seen)}; "
                f"no keyboard route out of it",
            )
            print(frame.dump())
            return frame
        before = frame.blob
        frame, moved = press_and_settle(
            vbox, key, snap,
            f"1{step + 1}-{frame.screen.replace('/', '-')}-{key}", before,
            ocr_deadline=settle,
        )
        if not moved:
            checks.that(
                "the setup journey reaches Home on the keyboard alone",
                False,
                f"{key} on {seen[-1]} changed nothing in {settle}s - the "
                f"control is drawn but not wired to the keyboard",
            )
            return frame
    checks.that(
        "the setup journey reaches Home on the keyboard alone",
        False,
        f"still not home after {max_steps} steps: {' -> '.join(seen)}",
    )
    return frame


def wait_for_render(vbox, out, ocr, deadline=90, interval=3):
    """Poll until something that is not the firmware is on screen.

    Sleeping a fixed number of seconds is what made every earlier arm64 run
    ambiguous: a failure at second 30 could be a slow boot or a dead kernel.
    This keeps every intermediate frame, so the report can show what the screen
    looked like while it was still firmware.
    """
    frames = []
    start = time.time()
    n = 0
    while time.time() - start < deadline:
        p = out / f"00-boot-{n:02d}.png"
        try:
            vbox.shot(p)
        except RuntimeError as e:
            print(f"  t+{time.time() - start:4.0f}s  no screen yet ({e.args[0][:60]})")
            time.sleep(interval)
            n += 1
            continue
        f = Frame(p, ocr)
        frames.append(f)
        print(f"  t+{time.time() - start:4.0f}s  {f.describe()}")
        # Prefer a screen that can actually be named. The pixel heuristic on
        # its own returned a frame at 1920x1080 that was 100% painted with 121
        # colours and no readable text yet - the compositor had cleared and
        # started drawing, and the harness called it done and then reported
        # "could not identify the screen". Only fall back to pixels once the
        # deadline is nearly up, so a guest with no OCR still gets an answer.
        if f.screen not in ("blank", "unknown", "unreadable"):
            return f, frames
        near_deadline = time.time() - start > deadline * 0.6
        if near_deadline and f.paint["fraction"] >= MIN_PAINTED and f.colours >= 40:
            return f, frames
        time.sleep(interval)
        n += 1
    return (frames[-1] if frames else None), frames


# --------------------------------------------------------------------------
# selftest
# --------------------------------------------------------------------------


class FakeFrame:
    """A screen described by its text, for testing the checks themselves."""

    def __init__(self, lines, w=1280, h=800, screen="brief"):
        self.w, self.h = w, h
        self.lines = [
            {"text": t, "x": x, "y": y, "w": max(8, len(t) * 7), "h": 14, "conf": 1.0}
            for (t, x, y) in lines
        ]
        self.texts = [l["text"] for l in self.lines]
        self.blob = " │ ".join(self.texts).lower()
        self.screen = screen


def _cases():
    """(name, check, screen text, must-go-red).

    Every check that cannot be provoked on the guest on demand has its verdict
    pinned here against the exact text the defect produces. This is the
    falsifiability proof for the predicate; scripts/e2e/fixtures/*.png is the
    proof against real frames. A check with no entry in one of the two is a
    check nobody has ever watched fail, and this project has shipped those.

    The green cases matter as much as the red ones: a check that fires on a
    correct screen gets weakened or deleted the first week.
    """
    def row(t, y):
        return (t, 296, y)

    return [
        # The bug reported on this guest: one document, two labels.
        ("duplicate Doc/Hit", check_no_duplicate_rows, [
            row("Goal: os", 262), row("Query: os", 314),
            row("Doc: os identity", 366), row("Hit: os identity", 418),
            row("Doc: Agent skills", 470), row("Hit: Agent skills", 522),
        ], True),
        # ...and it must still fire when Vision mangles one of the two titles,
        # which it demonstrably does: it read "paper" as "papel" on a real frame.
        ("duplicate survives an OCR slip", check_no_duplicate_rows, [
            row("Doc: os identity", 366), row("Hit: os identlty", 418),
        ], True),
        # Goal and Query are *supposed* to repeat the request.
        ("Goal and Query may repeat", check_no_duplicate_rows, [
            row("Goal: paper", 262), row("Query: paper", 314),
            row("Doc: os identity", 366),
        ], False),
        ("distinct results are fine", check_no_duplicate_rows, [
            row("Doc: os identity", 366), row("Doc: Agent skills", 418),
        ], False),
        # Near neighbours that are genuinely different documents must not be
        # collapsed, or the check becomes noise and gets switched off.
        ("similar but different documents", check_no_duplicate_rows, [
            row("Doc: thesis-draft.md", 366), row("Doc: thesis-final.md", 418),
        ], False),
        # "5 matches for nvda." above three cards - shipped.
        ("count above fewer rows", check_counts_agree, [
            row("5 matches for nvda.", 220),
            row("Doc: a", 262), row("Doc: b", 314), row("Doc: c", 366),
        ], True),
        ("count that matches", check_counts_agree, [
            row("3 matches for nvda.", 220),
            row("Doc: a", 262), row("Doc: b", 314), row("Doc: c", 366),
        ], False),
        # "Go Back" drawn below the bottom edge - shipped.
        ("control below the bottom edge", check_on_screen, [
            ("Go Back", 296, 792),
        ], True),
        ("control inside the page", check_on_screen, [
            ("Go Back", 296, 700),
        ], False),
        # The same explanation printed twice in two places - shipped.
        # Fixture uses standalone copy (KERNEL_FEATURES=standalone is the tip).
        ("one explanation, printed twice", check_no_repeated_sentence, [
            row("Local keywords only.", 220),
            row("Local keywords only.", 700),
        ], True),
        ("explanations that differ", check_no_repeated_sentence, [
            row("Local keywords only.", 220),
            row("No openable hits - refine the ask.", 700),
        ], False),
        # Dropped keystrokes: "i wanna work on my paper" arrived as
        # "i wanna wy paper", and only reading the text back revealed it.
        ("keystrokes dropped on the way in",
         lambda c, f: check_field_holds(c, f, "i wanna work on my paper"), [
             row("i wanna wy paper", 151),
         ], True),
        ("keystrokes arrived",
         lambda c, f: check_field_holds(c, f, "i wanna work on my paper"), [
             row("i wanna work on my paper", 151),
         ], False),
        # Doc rows above a sentence denying any exist - live on this guest now.
        ("Doc rows above 'No openable hits'", check_openable_claim, [
            row("Doc: os identity", 366),
            row("Info: No openable hits - refine the ask.", 418),
        ], True),
        ("no Doc rows, so the denial is honest", check_openable_claim, [
            row("Info: No openable hits - refine the ask.", 418),
        ], False),
        # The arm64 guest painted Home during boot and replaced it with setup
        # Welcome a second and a half later. Everything this harness drove after
        # that was aimed at a screen that no longer existed.
        ("the boot screen turns into another one",
         lambda c, f: check_first_screen_settles(
             c, FakeFrame([row("Search", 241)], screen="home"),
             lambda _n: FakeFrame([row("hello", 333)], screen="setup/welcome"),
             wait=0,
         ), [], True),
        ("the boot screen stays put",
         lambda c, f: check_first_screen_settles(
             c, FakeFrame([row("hello", 333)], screen="setup/welcome"),
             lambda _n: FakeFrame([row("hello", 333)], screen="setup/welcome"),
             wait=0,
         ), [], False),
    ]


def selfcheck():
    """Prove each predicate goes red on the defect it names, and only then."""
    cases = _cases()
    bad = 0
    print("predicate falsification:")
    for name, fn, lines, expect_red in cases:
        c = Checks()
        c.that = lambda n, ok, d="", _c=c: (
            _c.results.append((bool(ok), n, d)), bool(ok)
        )[1]
        c.skip = lambda n, w, _c=c: _c.results.append((None, n, w))
        fn(c, FakeFrame(lines))
        went_red = any(r[0] is False for r in c.results)
        ok = went_red == expect_red
        print(
            f"  {'OK   ' if ok else 'WRONG'} {name}: "
            f"expected {'red' if expect_red else 'green'}, "
            f"got {'red' if went_red else 'green'}"
            + ("" if ok else f"   {[r[1:] for r in c.results]}")
        )
        bad += 0 if ok else 1
    print(f"\nselfcheck: {len(cases) - bad}/{len(cases)} predicates behaved as specified")
    return 1 if bad else 0


def selftest(directory, ocr):
    """Re-run the checks over saved frames and require the stated verdict.

    A check that cannot go red is worse than no check, and this project has
    shipped those. Each fixture is a real screenshot with a filename that says
    what it is; the run fails if a fixture named `*.red.png` passes, or one
    named `*.green.png` fails. Capture fixtures by copying frames out of a
    normal run's --out directory.
    """
    d = Path(directory)
    shots = sorted(list(d.glob("*.red.png")) + list(d.glob("*.green.png")))
    if not shots:
        print(f"no fixtures in {d} (expected *.red.png / *.green.png)")
        return 1
    bad = 0
    for shot in shots:
        want_red = shot.name.endswith(".red.png")
        f = Frame(shot, ocr)
        c = Checks()
        print(f"\n--- {shot.name}: {f.describe()}")
        mode = None
        cfg = None
        meta = shot.with_suffix("").with_suffix(".json")
        if meta.exists():
            m = json.loads(meta.read_text())
            mode = tuple(m["mode"]) if m.get("mode") else None
            cfg = tuple(m["configured"]) if m.get("configured") else None
        check_rendering(c, f, cfg, mode)
        check_on_screen(c, f)
        check_no_duplicate_rows(c, f)
        check_openable_claim(c, f)
        check_counts_agree(c, f)
        check_no_repeated_sentence(c, f)
        went_red = any(r[0] is False for r in c.results)
        verdict = "red" if went_red else "green"
        want = "red" if want_red else "green"
        ok = verdict == want
        print(f"    fixture expects {want}, checks went {verdict}: {'OK' if ok else 'WRONG'}")
        bad += 0 if ok else 1
    print(f"\nselftest: {len(shots) - bad}/{len(shots)} fixtures behaved as labelled")
    return 1 if bad else 0


# --------------------------------------------------------------------------


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="/tmp/os-arm64-e2e")
    ap.add_argument("--iso", default=str(ROOT / "os-arm64.iso"))
    ap.add_argument("--vm", default=VM)
    ap.add_argument("--query", default="paper")
    ap.add_argument("--frontend", default="gui", choices=["gui", "headless", "separate"])
    ap.add_argument("--no-boot", action="store_true",
                    help="drive the VM that is already running")
    ap.add_argument("--create", action="store_true",
                    help="create --vm if it does not exist, with the settings "
                         "scripts/make-virtualbox-arm64.sh uses")
    ap.add_argument("--boot-deadline", type=int, default=90)
    ap.add_argument("--settle", type=int, default=30,
                    help="seconds to wait for the screen to answer a keypress "
                         "before calling the control unwired")
    ap.add_argument("--selftest", metavar="DIR", nargs="?",
                    const=str(HERE / "fixtures"),
                    help="re-check saved frames instead of booting "
                         "(default: scripts/e2e/fixtures)")
    ap.add_argument("--selfcheck", action="store_true",
                    help="prove each check goes red on the defect it names")
    args = ap.parse_args()

    ocr = Ocr()
    if not ocr.available:
        print(f"warning: OCR unavailable - {ocr.why}")
        print("         pixel checks still run; every text check will SKIP.")

    if args.selfcheck:
        return selfcheck()

    if args.selftest:
        return selfcheck() | selftest(args.selftest, ocr)

    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    vbox = Vbox(args.vm)
    if vbox.state() == "absent":
        if not args.create:
            raise SystemExit(
                f"no VM named {args.vm}. Create it with "
                f"scripts/make-virtualbox-arm64.sh, or pass --create."
            )
        print(f"creating {args.vm}")
        vbox.create()

    info = vbox.info()
    # What the *ISO* asks Limine for. This is the mode that actually decides
    # the guest's framebuffer; see iso_resolution().
    configured = iso_resolution(args.iso)
    # Firmware start-up mode, reported for context only. Measured: changing it
    # does not change the mode the guest ends up in.
    res = vbox.manage(
        "getextradata", args.vm, "VBoxInternal2/EfiGraphicsResolution", check=False
    )
    m = re.search(r"(\d+)x(\d+)", res)
    efi = (int(m.group(1)), int(m.group(2))) if m else None

    print(f"vm: {args.vm}  chipset={info.get('chipset')} "
          f"gfx={info.get('graphicscontroller')} vram={info.get('vram')}MB "
          f"uart1={info.get('uart1')}")
    print(f"ISO limine resolution: "
          f"{f'{configured[0]}x{configured[1]}' if configured else '(none - Limine chooses)'}"
          f"   firmware EfiGraphicsResolution: "
          f"{f'{efi[0]}x{efi[1]}' if efi else '(unset)'} (context only)")

    if not args.no_boot:
        iso = Path(args.iso)
        if not iso.exists():
            raise SystemExit(f"{iso} missing - run 'make arm64-iso'")
        print(f"booting {iso.name} ({iso.stat().st_size} bytes)")
        vbox.poweroff()
        vbox.attach_iso(iso)
        vbox.start(args.frontend)
    else:
        print("--no-boot: driving whatever is already on screen")

    checks = Checks()
    print("\nwaiting for the guest to render:")
    first, boot_frames = wait_for_render(vbox, out, ocr, args.boot_deadline)
    if first is None:
        print("the VM never produced a screenshot at all")
        return 1

    boot_key = vbox.session_key()

    def snap(name):
        now = vbox.session_key()
        if now != boot_key:
            raise VmRestarted(
                f"the VM was power-cycled underneath this run "
                f"(boot {boot_key} -> {now}). Another process is driving "
                f"{args.vm}; run with --vm <other-name> --create to get one "
                f"to yourself. Nothing below this point is about the product."
            )
        f = Frame(vbox.shot(out / f"{name}.png"), ocr)
        print(f"  shot {name}.png  {f.describe()}")
        return f

    mode = guest_mode_from_log(vbox.log_path())
    print(f"\nVirtualBox Display last accepted: "
          f"{f'{mode[0]}x{mode[1]}x{mode[2]}' if mode else '(not in log)'}")

    print("\nchecks - boot and framebuffer:")
    # A precondition, not a product claim, but it belongs in the report: with
    # uart1 off this guest boots and then ignores every key, and the run would
    # otherwise blame the UI for it.
    checks.that(
        "the VM has the serial port the guest needs to finish booting",
        info.get("uart1", "off") != "off",
        "uart1=off. The guest will render Home and then accept no input at "
        "all. Run: VBoxManage modifyvm %s --uart1 0x3f8 4 --uart-mode1 file "
        "/tmp/%s-uart.log" % (args.vm, args.vm),
    )
    check_rendering(checks, first, configured, mode)
    check_on_screen(checks, first)
    if not args.no_boot:
        check_first_screen_settles(checks, first, snap)

    try:
        print("\ndriving to Home:")
        home = drive_to_home(vbox, snap, checks, settle=args.settle)

        if home.screen == "home":
            print("\nchecks - home:")
            check_on_screen(checks, home)
            check_no_repeated_sentence(checks, home)

            print(f"\ntyping {args.query!r}:")
            vbox.type(args.query)
            time.sleep(2.0)
            typed = snap("20-typed")
            check_field_holds(checks, typed, args.query)

            answer, moved = press_and_settle(
                vbox, "enter", snap, "21-answer", typed.blob,
                ocr_deadline=args.settle,
            )
            checks.that(
                "Enter on Home runs something and leaves Home",
                moved,
                f"the screen never changed in {args.settle}s - Enter is not "
                f"wired on Home",
            )
            print("\nchecks - the screen Enter produced:")
            check_rendering(checks, answer, configured, mode)
            check_on_screen(checks, answer)
            check_no_duplicate_rows(checks, answer)
            check_openable_claim(checks, answer)
            check_counts_agree(checks, answer)
            check_no_repeated_sentence(checks, answer)
            rows = brief_rows(answer)
            print(f"  rows read off the screen ({len(rows)}):")
            for tag, text in rows:
                print(f"    {tag}: {text}")
        else:
            checks.skip("home checks", f"never reached Home (stopped on {home.screen})")
    except VmRestarted as e:
        print(f"\nABORTED: {e}")
        print("  Not counting anything after the restart. This run proves "
              "nothing about the product.")
        checks.report()
        return 2

    # Metadata beside the frames so --selftest can re-check them later without
    # the VM.
    (out / "run.json").write_text(json.dumps({
        "vm": args.vm,
        "mode": list(mode) if mode else None,
        "configured": list(configured) if configured else None,
        "query": args.query,
    }, indent=2))

    rc = checks.report()
    print(f"\nframes in {out}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
