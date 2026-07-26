# `make e2e` — screen-level invariants

Boots the real ISO, works out where it is from what is drawn, drives it, and
asserts against the rendered result.

```
make e2e                              # full run + the proof each assertion can fail
make e2e E2E_OUT=/tmp/shots           # where the evidence lands (default /tmp/os-e2e)
make e2e E2E_ARGS="--no-sweep"        # skip the click-everything wiring sweep
make e2e E2E_ARGS="--no-prove"        # skip the self-proof
./scripts/e2e/invariants.py --help    # every knob
```

Exits non-zero if any assertion fails, **or** if any assertion stays green when
its evidence is deliberately broken.

## Why it is not part of `make test`

`make test` is the fast loop: unit tests, a QEMU smoke boot, a bridge smoke.
`make e2e` needs QEMU, a live host bridge on `127.0.0.1:7420`, macOS's Vision
text recogniser, and about four minutes. Wiring that into the fast loop means
people stop running the fast loop. Run `make e2e` before shipping a UI change,
and after any redesign that moves a control.

Requirements: macOS (Vision, via `scripts/e2e/screentext.swift`, built on first
use), `qemu-system-x86_64`, `numpy`, and `sips`.

## What it asserts

Each line is one named assertion. On failure it prints what was expected, what
was seen, and the screenshot it was seen on.

| Assertion | Bug class it exists to catch |
|---|---|
| the kernel never reported a CPU fault | boot health |
| the kernel never panicked | boot health |
| the setup journey ends on the home screen | a journey with no way out |
| every setup step offers a primary action that is on screen | 2 — the Continue pill below the fold |
| no control is cut off by the edge of the screen | 2 — drawn off-screen |
| no two controls on a screen partially overlap | 2 — one control on top of another |
| no text runs off the edge of the screen | 2 |
| every control that looks clickable does something | 1 — drawn but not wired |
| the query field shows exactly what was typed | 4 — dropped keystrokes |
| every keystroke typed reaches the bridge unaltered | 4 |
| a count stated in prose equals the rows rendered | 3 — "5 matches" above three cards |
| no result list shows the same row twice | 7 — the arm64 `Doc:`/`Hit:` duplication |
| no screen explains the same thing twice | 6 |
| prose about the source agrees with where the rows came from | 5 — "bridge offline" above bridge results |
| clicking the search field leaves you on the home screen | 8 — the field restarting setup |

## Why it does not break when the UI is redesigned

Nothing here knows where anything is. The harness that did — `drive-ui.py` —
broke on every redesign and blamed the product each time.

- **Which screen am I on?** The nav title strip, read with OCR, checked against
  the titles scraped out of `screens.rs` / `searchui.rs`. New screens turn up on
  the next run for free.
- **Where is the primary action?** The one *filled* block of `theme::ACCENT`.
- **Where are the rows?** Controls outlined in `theme::CARD_BORDER`.
- **Where is the query box?** The wide control outlined in `theme::RULE`.
- **Is that capability on?** The switch track is `ACCENT` when on and `RULE`
  when off, so the harness reads it instead of assuming — the first version of
  this suite "granted" a capability that was already on and un-granted it.
- **What colour is `ACCENT`?** Parsed out of `kernel/src/ui.rs`, never copied.

The one thing it does assume is the palette's *meaning*: that a filled accent
block is the primary action and a card border outlines a row. Change that and
this file must change with it.

## Proving the assertions can fail

A test that cannot fail is worse than none, and this project has shipped those.

1. `--prove` (on by default) takes the evidence the run just collected, breaks
   one thing in it per assertion — inserts a `FAULT` line, moves a control past
   the bottom edge, repeats a row under a `Hit` tag, states a count two higher
   than the rows drawn, drops a keystroke — and requires that assertion to go
   red. An assertion that stays green is reported as a failure of the suite.
2. Evidence-mutation only proves the logic. To prove the layout assertions are
   wired to the real product, build a genuinely broken ISO and run against it:

   ```
   make iso IMAGE_NAME=os-lowres RESOLUTION=640x480
   ./scripts/e2e/invariants.py --iso os-lowres.iso --out /tmp/e2e-lowres --no-prove
   ```

   The screen layouts use fixed offsets (`screens.rs` `TOP = 150`, `ROW_H = 62`),
   so at 640x480 the fifth Brief row is cut off by the bottom edge and
   *"no control is cut off by the edge of the screen"* goes red — on the same
   sources that are green at 1280x800.

## Known limits — read these before trusting a green run

- **The duplicate-row assertion has only been run green on x86.** The live
  `Doc:`/`Hit:` duplication was seen on the arm64 VirtualBox guest, which has no
  QMP and no serial and cannot be driven from here. On x86 with a live bridge
  the intent hits carry URLs, so `agent.rs`'s `Hit` fallback (the branch that
  re-lists the same titles when `doc_n` never moved) does not run. The assertion
  is proven able to catch that shape — `--prove` repeats a real row under a
  `Hit` tag and it goes red — but it is **unverified against the screen where
  the bug was actually seen**.
- **"A count stated in prose equals the rows rendered" is vacuous when no
  screen states a count.** The run says so out loud (`no count stated in prose
  on any result screen this run`) rather than quietly passing.
- **The bridge is shared.** `scripts/ensure-bridge.sh` truncates `.bridge.log`
  and replaces the process. If something else restarts the bridge mid-run the
  guest looks mute; the keystroke assertion says so explicitly instead of
  blaming the product.
- **The wiring sweep only covers the home screen.** Rows inside Skills,
  Capabilities and Brief are not clicked.
- **Self-contained on purpose.** Another session is building `theme.py`,
  `atlas.py`, `catalog.py`, `frame.py`, `screen.py`, `driver.py` and `arm64.py`
  in this directory, plus its own `ocr.swift`. `invariants.py` imports none of
  them and uses `screentext.swift` so the two cannot silently become each other.
  Merge them deliberately later.
