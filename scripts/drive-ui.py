#!/usr/bin/env python3
"""Boot the ISO, drive it, and photograph the screen.

This is now a thin front door onto `scripts/e2e/driver.py`. The old body of
this file walked a hardcoded journey - click Continue six times, then click
(640, 158) - and every redesign broke it in a way that looked like a product
bug: the placeholder changed, Enter started running an agent Brief instead of a
search, Status and Brief screens appeared, the capability rows moved twice,
setup got shorter. The replacement derives its state from the framebuffer, so
those are all just Tuesday.

Kept as a name because the Makefile and the notes point at it, and because two
half-maintained drivers is worse than either one.

    ./scripts/drive-ui.py --out /tmp/shots --query nvda
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from scripts.e2e.driver import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
