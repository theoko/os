//! First-boot setup journey, in the shape of the macOS Setup Assistant.
//!
//! One decision per screen, centred on a white page, with a single primary
//! action and a quiet way back. The steps mirror what this OS actually has to
//! establish before the agent can do anything: where it is, which capabilities
//! are granted, and which skills load.
//!
//! Drawing records its own hit zones, so `click()` needs no separate layout
//! table to drift out of sync.

use crate::caps::Caps;
use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, HERO_FACE, SMALL_FACE, TITLE_FACE};
use crate::keyboard::Key;
use crate::level::Level;
use crate::mcp::MailPeek;
use crate::skills::SkillPeek;
use crate::ui::theme;

/// Where a click landed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Continue,
    Back,
    /// A selectable/toggleable row.
    Row(usize),
}

#[derive(Clone, Copy, Debug)]
struct Zone {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    action: Action,
}

impl Zone {
    fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// The journey, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Welcome,
    /// Explicit Guided vs Advanced — copy adapts; grants stay privacy-first.
    Experience,
    Region,
    Capabilities,
    Skills,
    Done,
    /// Setup finished; the home screen takes over.
    Finished,
}

pub const REGIONS: [&str; 4] = ["United States", "United Kingdom", "Greece", "Japan"];

/// Capabilities the agent may be granted up front. Mirrors [`Cap`] / bridge tools.
/// Screen rows come straight from `Cap::ALL`, so a label can never drift from
/// the capability it grants — they used to be two lists kept in step by hand.
pub fn cap_rows(level: Level) -> impl Iterator<Item = (&'static str, &'static str)> {
    crate::caps::Cap::ALL
        .iter()
        .map(move |c| (c.label(), c.blurb(level)))
}

/// Number of capability rows.
pub const N_CAPS: usize = crate::caps::Cap::ALL.len();

const MAX_ZONES: usize = 12;
const NO_FOCUS: usize = usize::MAX;

pub struct Setup {
    pub step: Step,
    pub level: Level,
    pub region: usize,
    pub caps: [bool; N_CAPS],
    zones: [Zone; MAX_ZONES],
    n_zones: usize,
    focus: usize,
    keyboard_focus: bool,
    /// Pointer target under the cursor. Kept as the action rather than a zone
    /// index because drawing rebuilds the zone table on every frame.
    hovered: Option<Action>,
    /// Edge detection: a held button must not advance every frame.
    was_down: bool,
    /// Which input devices are missing, in words, or `None` when the machine
    /// is fully driveable. Welcome is the only screen a machine with no
    /// keyboard and no pointer will ever show, so it has to carry the message.
    note: Option<&'static str>,
}

impl Setup {
    pub const fn new() -> Self {
        Self {
            step: Step::Welcome,
            level: Level::Guided,
            region: 0,
            // Privacy first: nothing is on that reads personal data, writes
            // to disk, or leaves the machine. Only the docs that ship with the
            // OS are searchable out of the box.
            //
            // Order matches caps::Cap::ALL. Level never changes these defaults.
            // Email / Send mail / docs / files / recordings / save / portals
            caps: [false, false, true, false, false, false, false],
            zones: [Zone {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                action: Action::Continue,
            }; MAX_ZONES],
            n_zones: 0,
            focus: NO_FOCUS,
            keyboard_focus: false,
            hovered: None,
            was_down: false,
            note: None,
        }
    }

    /// Restart the journey but keep the last Experience choice selected, and
    /// the input note: a machine that was missing a pointer a minute ago is
    /// still missing it now.
    pub fn restart(level: Level, note: Option<&'static str>) -> Self {
        let mut s = Self::new();
        s.level = level;
        s.note = note;
        s
    }

    /// Tell setup which input devices did not come up. See [`Self::note`].
    pub fn set_note(&mut self, note: Option<&'static str>) {
        self.note = note;
    }

    /// Grant set chosen on the Capabilities step.
    pub fn grants(&self) -> Caps {
        Caps::from_bools(&self.caps)
    }

    pub fn is_finished(&self) -> bool {
        self.step == Step::Finished
    }

    fn reset_zones(&mut self) {
        self.n_zones = 0;
    }

    fn push_zone(&mut self, x: i32, y: i32, w: i32, h: i32, action: Action) {
        if self.n_zones < MAX_ZONES {
            self.zones[self.n_zones] = Zone { x, y, w, h, action };
            self.n_zones += 1;
        }
    }

    fn hit(&self, px: i32, py: i32) -> Option<Action> {
        self.zones[..self.n_zones]
            .iter()
            .find(|z| z.contains(px, py))
            .map(|z| z.action)
    }

    /// Update the visible pointer target without activating it.
    ///
    /// This is separate from [`pointer`](Self::pointer) so the main loop can
    /// redraw a hover affordance without playing a screen transition.
    pub fn pointer_hover(&mut self, x: i32, y: i32) -> bool {
        let next = self.hit(x, y);
        if next == self.hovered {
            return false;
        }
        self.hovered = next;
        self.keyboard_focus = false;
        true
    }

    /// Feed pointer state. Returns true when the screen needs redrawing.
    ///
    /// Only the press *edge* counts — holding the button down must not run the
    /// whole journey in a few frames.
    pub fn pointer(&mut self, x: i32, y: i32, buttons: u8) -> bool {
        let down = buttons & 0x01 != 0;
        let pressed = down && !self.was_down;
        self.was_down = down;
        if !pressed {
            return false;
        }
        self.keyboard_focus = false;
        match self.hit(x, y) {
            Some(action) => self.apply(action),
            None => false,
        }
    }

    /// Drive setup without a pointer.
    ///
    /// Tab/Down and Up move a visible focus ring, Enter activates it, and
    /// Escape goes back. The default focus is Continue, so first boot can be
    /// completed with Enter alone when the defaults are acceptable.
    pub fn key(&mut self, key: Key) -> bool {
        if self.n_zones == 0 {
            return false;
        }
        match key {
            Key::Tab | Key::Down => {
                self.keyboard_focus = true;
                self.focus = (self.focus + 1) % self.n_zones;
                true
            }
            Key::Up => {
                self.keyboard_focus = true;
                self.focus = if self.focus == 0 {
                    self.n_zones - 1
                } else {
                    self.focus - 1
                };
                true
            }
            Key::Enter | Key::Char(b' ') => {
                self.keyboard_focus = true;
                let action = self.zones[self.focus.min(self.n_zones - 1)].action;
                self.apply(action)
            }
            Key::Escape => self.apply(Action::Back),
            _ => false,
        }
    }

    /// Apply an action. Returns true when the screen changed.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Continue => {
                self.step = match self.step {
                    Step::Welcome => Step::Experience,
                    Step::Experience => Step::Region,
                    Step::Region => Step::Capabilities,
                    Step::Capabilities => Step::Skills,
                    Step::Skills => Step::Done,
                    Step::Done => Step::Finished,
                    Step::Finished => Step::Finished,
                };
                self.focus = NO_FOCUS;
                self.hovered = None;
                true
            }
            Action::Back => {
                self.step = match self.step {
                    Step::Welcome | Step::Experience => Step::Welcome,
                    Step::Region => Step::Experience,
                    Step::Capabilities => Step::Region,
                    Step::Skills => Step::Capabilities,
                    Step::Done => Step::Skills,
                    Step::Finished => Step::Finished,
                };
                self.focus = NO_FOCUS;
                self.hovered = None;
                true
            }
            Action::Row(i) => match self.step {
                Step::Experience => {
                    if i < Level::ALL.len() {
                        self.level = Level::ALL[i];
                        return true;
                    }
                    false
                }
                Step::Region => {
                    if i < REGIONS.len() {
                        self.region = i;
                        return true;
                    }
                    false
                }
                Step::Capabilities => {
                    if i < N_CAPS {
                        self.caps[i] = !self.caps[i];
                        return true;
                    }
                    false
                }
                _ => false,
            },
        }
    }

    /// Paint the current step. Records hit zones as a side effect.
    pub fn draw(&mut self, fb: &Surface, _mail: &MailPeek, skills: &SkillPeek) {
        let w = fb.width() as i32;
        let h = fb.height() as i32;
        fb.fill(theme::BG);
        self.reset_zones();

        match self.step {
            Step::Welcome => self.draw_welcome(fb, w, h),
            Step::Experience => self.draw_experience(fb, w, h),
            Step::Region => self.draw_region(fb, w, h),
            Step::Capabilities => self.draw_caps(fb, w, h),
            Step::Skills => self.draw_skills(fb, w, h, skills),
            Step::Done => self.draw_done(fb, w, h),
            Step::Finished => {}
        }
        self.draw_progress(fb, w);
        if self.n_zones > 0 {
            if self.focus >= self.n_zones {
                self.focus = self.zones[..self.n_zones]
                    .iter()
                    .position(|z| z.action == Action::Continue)
                    .unwrap_or(0);
            }
            if self.keyboard_focus {
                self.draw_focus(fb);
            }
        }
    }

    fn draw_focus(&self, fb: &Surface) {
        let zone = self.zones[self.focus.min(self.n_zones - 1)];
        let x = zone.x - 3;
        let y = zone.y - 3;
        let w = zone.w + 6;
        let h = zone.h + 6;
        fb.draw_round_rect_outline(x, y, w, h, 12, 2, theme::ACCENT);
    }

    /// A quiet setup rail: completed steps stay blue, the current step is
    /// larger, and the remaining path stays grey. It answers "where am I?"
    /// without adding another sentence to every screen.
    fn draw_progress(&self, fb: &Surface, w: i32) {
        let active = match self.step {
            Step::Welcome | Step::Finished => return,
            Step::Experience => 1,
            Step::Region => 2,
            Step::Capabilities => 3,
            Step::Skills => 4,
            Step::Done => 5,
        };
        const STEPS: i32 = 6;
        const PITCH: i32 = 18;
        let x0 = w / 2 - (STEPS - 1) * PITCH / 2;
        let cy = 28;
        for i in 0..STEPS {
            let current = i == active;
            let d = if current { 8 } else { 5 };
            let color = if i <= active {
                theme::ACCENT
            } else {
                theme::RULE
            };
            fb.fill_round_rect(x0 + i * PITCH - d / 2, cy - d / 2, d, d, d / 2, color);
        }
    }

    fn draw_welcome(&mut self, fb: &Surface, w: i32, h: i32) {
        // Apple opens on a single word and nothing else.
        let track = font::tracking_pct(HERO_FACE.px, -30);
        let cy = h / 2 - 40;
        fb.draw_text_centered(w / 2, cy, "hello", &HERO_FACE, track, theme::INK);
        self.primary(fb, w, cy + 90, "Continue");
        // The top bar is empty on this step — `nav_back` and `draw_progress`
        // both bow out of Welcome — so the note goes there rather than near
        // the button, where it would fight the footer for space on a short
        // framebuffer.
        if let Some(note) = self.note {
            fb.draw_text_centered(
                w / 2,
                28 + SMALL_FACE.baseline(),
                note,
                &SMALL_FACE,
                0,
                theme::MUTED,
            );
        }
    }

    fn draw_experience(&mut self, fb: &Surface, w: i32, h: i32) {
        let top = self.header(
            fb,
            w,
            h,
            "How should it feel?",
            "You choose. Grants stay off until you turn them on.",
        );
        let mut y = top;
        for (i, level) in Level::ALL.iter().enumerate() {
            self.row(
                fb,
                w,
                y,
                ROW_H,
                level.label(),
                Some(level.detail()),
                self.level == *level,
                false,
                Action::Row(i),
            );
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y, true);
    }

    fn draw_region(&mut self, fb: &Surface, w: i32, h: i32) {
        let top = self.header(
            fb,
            w,
            h,
            "Select Your Region",
            "This sets formatting defaults. It does not leave the machine.",
        );
        let sel = self.region;
        let mut y = top;
        for (i, name) in REGIONS.iter().enumerate() {
            self.row(fb, w, y, ROW_H, name, None, i == sel, false, Action::Row(i));
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y, true);
    }

    fn draw_caps(&mut self, fb: &Surface, w: i32, h: i32) {
        let sub = if self.level.is_guided() {
            "Every tool sits behind a grant. Turn on only what you need."
        } else {
            "Consent switches. Defaults stay privacy-first."
        };
        let top = self.header(fb, w, h, "Capabilities", sub);
        let mut y = top;
        let pitch = row_pitch(top, h, cap_rows(self.level).count(), true);
        for (i, (name, blurb)) in cap_rows(self.level).enumerate() {
            let on = self.caps[i];
            self.row(
                fb,
                w,
                y,
                pitch - ROW_GAP,
                name,
                Some(blurb),
                on,
                true,
                Action::Row(i),
            );
            y += pitch;
        }
        self.footer(fb, w, h, y - pitch + ROW_H, true);
    }

    fn draw_skills(&mut self, fb: &Surface, w: i32, h: i32, skills: &SkillPeek) {
        let sub = if skills.from_bridge {
            if self.level.is_guided() {
                "Live from the host bridge. Tap Continue when ready."
            } else {
                "skills.list from bridge."
            }
        } else if self.level.is_guided() {
            "Markdown playbooks the agent can load. Editable later."
        } else {
            crate::copy::setup_skills_builtin()
        };
        let top = self.header(fb, w, h, "Default Skills", sub);
        let mut y = top;
        // All builtins fit at ROW_H=46 on a 768 screen (see layout_tests); on a
        // shorter one row_pitch tightens them so the footer stays reachable.
        let n = skills.count.min(7);
        let pitch = row_pitch(top, h, n, true);
        for i in 0..n {
            let desc = skills.desc_at(i);
            let blurb = if desc.is_empty() { None } else { Some(desc) };
            self.row(
                fb,
                w,
                y,
                pitch - ROW_GAP,
                skills.name_at(i),
                blurb,
                true,
                false,
                Action::Row(i),
            );
            y += pitch;
        }
        self.footer(fb, w, h, y - pitch.min(y) + ROW_H, true);
    }

    fn draw_done(&mut self, fb: &Surface, w: i32, h: i32) {
        self.nav_back(fb);
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        let cy = h / 2 - 40;
        fb.draw_text_centered(w / 2, cy, "You're all set.", &TITLE_FACE, track, theme::INK);
        fb.draw_text_centered(
            w / 2,
            cy + 40,
            self.level.done_subtitle(),
            &BODY_FACE,
            0,
            theme::MUTED,
        );
        self.primary(fb, w, cy + 90, "Start");
        self.back_link(fb, w, cy + 90 + CTA_H + 30);
    }

    // --- shared chrome -----------------------------------------------------

    /// Title + subtitle. Returns the y where content should start.
    fn header(&mut self, fb: &Surface, w: i32, h: i32, title: &str, sub: &str) -> i32 {
        let _ = h;
        self.nav_back(fb);
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        let y = 132;
        fb.draw_text_centered(w / 2, y, title, &TITLE_FACE, track, theme::INK);
        fb.draw_text_centered(w / 2, y + 36, sub, &BODY_FACE, 0, theme::MUTED);
        y + 76
    }

    /// Keep navigation in the empty top bar. On a capability form it must not
    /// compete with the final switch or the primary action below the list.
    fn nav_back(&mut self, fb: &Surface) {
        if self.step == Step::Welcome {
            return;
        }
        let x = 32;
        let y = 28;
        fb.draw_text(
            x,
            y + BTN_FACE.baseline(),
            "Back",
            &BTN_FACE,
            0,
            theme::ACCENT,
        );
        if self.hovered == Some(Action::Back) {
            fb.fill_rect(
                x,
                y + BTN_FACE.px + 3,
                BTN_FACE.width("Back", 0),
                2,
                theme::TINT_BORDER,
            );
        }
        self.push_zone(
            x - 12,
            y - 10,
            BTN_FACE.width("Back", 0) + 24,
            BTN_FACE.px + 20,
            Action::Back,
        );
    }

    /// A selectable row. `toggle` draws a switch instead of a checkmark.
    #[allow(clippy::too_many_arguments)]
    /// One list row, `rh` tall.
    ///
    /// The height is passed in rather than fixed: on a short screen six rows
    /// at the ideal height cannot fit, and a fixed height meant they were
    /// drawn on top of each other instead of simply being smaller.
    fn row(
        &mut self,
        fb: &Surface,
        w: i32,
        y: i32,
        rh: i32,
        title: &str,
        blurb: Option<&str>,
        on: bool,
        toggle: bool,
        action: Action,
    ) {
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;

        let border = if on && !toggle {
            theme::ACCENT
        } else if self.hovered == Some(action) {
            theme::TINT_BORDER
        } else {
            theme::CARD_BORDER
        };
        let inner = if on && !toggle {
            theme::TINT_BG
        } else if self.hovered == Some(action) {
            theme::TINT_BG
        } else {
            theme::BG
        };
        fb.draw_round_rect_outline(x, y, cw, rh, 10, 1, border);
        fb.fill_round_rect(x + 1, y + 1, cw - 2, rh - 2, 9, inner);

        // Single baseline: label left, consequence beside it in muted grey.
        // Stacking a subtitle under every row made six of them overflow the
        // window, and the second line was never the thing being decided.
        let pad = 16;
        let base = y + rh / 2 + BRAND_FACE.px / 3;
        fb.draw_text(x + pad, base, title, &BRAND_FACE, 0, theme::INK);
        if let Some(b) = blurb {
            let lx = x + pad + BRAND_FACE.width(title, 0) + 12;
            fb.draw_text(lx, base, b, &SMALL_FACE, 0, theme::MUTED);
        }

        if toggle {
            let tw = 36;
            let th = 20;
            let tx = x + cw - pad - tw;
            let ty = y + (rh - th) / 2;
            let track_col = if on { theme::ACCENT } else { theme::RULE };
            fb.fill_round_rect(tx, ty, tw, th, th / 2, track_col);
            let knob = th - 6;
            let kx = if on { tx + tw - knob - 3 } else { tx + 3 };
            fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::BG);
        } else if on {
            let d = 9;
            fb.fill_round_rect(
                x + cw - pad - d,
                y + (rh - d) / 2,
                d,
                d,
                d / 2,
                theme::ACCENT,
            );
        }

        self.push_zone(x, y, cw, rh, action);
    }

    fn status_card(&mut self, fb: &Surface, w: i32, y: i32, label: &str, detail: &str, tint: u32) {
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;
        let card_h = ROW_H + 8;
        fb.draw_round_rect_outline(x, y, cw, card_h, 10, 1, theme::CARD_BORDER);
        fb.fill_round_rect(x + 1, y + 1, cw - 2, card_h - 2, 9, theme::BG);
        let pad = 18;
        let d = 9;
        fb.fill_round_rect(x + pad, y + (card_h - d) / 2, d, d, d / 2, tint);
        fb.draw_text(x + pad + d + 12, y + 26, label, &BRAND_FACE, 0, theme::INK);
        fb.draw_text(
            x + pad + d + 12,
            y + 46,
            detail,
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
    }

    /// Primary pill, centred, registering a Continue zone.
    fn primary(&mut self, fb: &Surface, w: i32, y: i32, label: &str) {
        let pad = 40;
        let bw = (BTN_FACE.width(label, 0) + pad * 2).max(180);
        let x = w / 2 - bw / 2;
        if self.hovered == Some(Action::Continue) {
            fb.fill_round_rect(
                x - 4,
                y - 4,
                bw + 8,
                CTA_H + 8,
                CTA_H / 2 + 4,
                theme::TINT_BORDER,
            );
        }
        fb.fill_round_rect(x, y, bw, CTA_H, CTA_H / 2, theme::ACCENT);
        let base = y + (CTA_H - BTN_FACE.px) / 2 + BTN_FACE.baseline() - 2;
        fb.draw_text_centered(w / 2, base, label, &BTN_FACE, 0, theme::BG);
        self.push_zone(x, y, bw, CTA_H, Action::Continue);
    }

    fn back_link(&mut self, fb: &Surface, w: i32, y: i32) {
        let label = "Go Back";
        let tw = BTN_FACE.width(label, 0);
        let x = w / 2 - tw / 2;
        fb.draw_text(
            x,
            y + BTN_FACE.baseline(),
            label,
            &BTN_FACE,
            0,
            theme::ACCENT,
        );
        if self.hovered == Some(Action::Back) {
            fb.fill_rect(x, y + BTN_FACE.px + 3, tw, 2, theme::TINT_BORDER);
        }
        // Generous target: the text alone is a 15px-tall sliver.
        self.push_zone(x - 12, y - 8, tw + 24, BTN_FACE.px + 20, Action::Back);
    }

    /// `content_bottom` = y just below the last row/card: on short
    /// framebuffers the pill moves down rather than overlapping the rows
    /// (zones are hit first-match, so an overlap misroutes clicks).
    fn footer(&mut self, fb: &Surface, w: i32, h: i32, content_bottom: i32, _back: bool) {
        // Push down to clear the content, but never past the bottom edge.
        //
        // `(h - 150).max(content_bottom + 24)` let tall content win with
        // nothing stopping it. On Capabilities - six rows, the longest screen
        // in the journey - that pinned Continue to the last visible pixels and
        // pushed "Go Back" up under the final row where it could not be seen,
        // so the only way out of the step was forward.
        let top = h - footer_height(false) - EDGE_MARGIN;
        let y = (content_bottom + 24).min(top).max(0);
        self.primary(fb, w, y, "Continue");
    }
}

/// Vertical pitch for a list of `n` rows starting at `top`, on a screen of
/// height `h` that still has to fit a footer.
///
/// Clamping the footer onto the screen only moved the problem: with six
/// capability rows the footer landed *on top of* the last one. Rows have to
/// give up the space, so the pitch tightens until the list fits.
fn row_pitch(top: i32, h: i32, n: usize, back: bool) -> i32 {
    let ideal = ROW_H + ROW_GAP;
    if n == 0 {
        return ideal;
    }
    let available = h - footer_height(back) - EDGE_MARGIN - 24 - top;
    (available / n as i32).clamp(MIN_ROW_PITCH, ideal)
}

/// Below this a row cannot hold its label, so a truly tiny screen clips rather
/// than rendering a list nobody can read.
const MIN_ROW_PITCH: i32 = 30;

/// Gap between rows; the rest of the pitch is the row itself.
const ROW_GAP: i32 = 8;

/// Gap between the back link and the primary action above it.
const BACK_GAP: i32 = 32;

/// Room the footer needs below `content_bottom`, back link included.
fn footer_height(back: bool) -> i32 {
    CTA_H + if back { BACK_GAP } else { 0 }
}

/// Breathing room below the primary action.
const EDGE_MARGIN: i32 = 20;

const CONTENT_W: i32 = 520;
const ROW_H: i32 = 46;
const CTA_H: i32 = 44;

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Setup {
        Setup::new()
    }

    #[test]
    fn journey_runs_forward_to_finished() {
        let mut s = setup();
        let order = [
            Step::Welcome,
            Step::Experience,
            Step::Region,
            Step::Capabilities,
            Step::Skills,
            Step::Done,
            Step::Finished,
        ];
        for (i, expected) in order.iter().enumerate() {
            assert_eq!(s.step, *expected, "step {i}");
            s.apply(Action::Continue);
        }
        assert!(s.is_finished());
    }

    #[test]
    fn back_walks_the_journey_in_reverse() {
        let mut s = setup();
        for _ in 0..4 {
            s.apply(Action::Continue);
        }
        assert_eq!(s.step, Step::Skills);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Capabilities);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Region);
    }

    #[test]
    fn experience_selection_sticks() {
        let mut s = setup();
        s.step = Step::Experience;
        assert_eq!(s.level, Level::Guided);
        assert!(s.apply(Action::Row(1)));
        assert_eq!(s.level, Level::Advanced);
        assert!(!s.apply(Action::Row(99)));
        assert_eq!(s.level, Level::Advanced);
    }

    #[test]
    fn level_never_changes_default_grants() {
        use crate::caps::Cap;
        for level in Level::ALL {
            let mut s = setup();
            s.level = level;
            let g = s.grants();
            assert!(g.allows(Cap::SearchQuery));
            assert!(!g.allows(Cap::EmailSearch));
            assert!(!g.allows(Cap::PortalSync));
        }
    }

    #[test]
    fn back_from_welcome_stays_put() {
        let mut s = setup();
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Welcome, "must not walk off the front");
    }

    #[test]
    fn finished_is_terminal() {
        let mut s = setup();
        s.step = Step::Finished;
        s.apply(Action::Continue);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Finished);
    }

    #[test]
    fn region_selection_sticks() {
        let mut s = setup();
        s.step = Step::Region;
        assert!(s.apply(Action::Row(2)));
        assert_eq!(s.region, 2);
        // Out of range must not panic or change anything.
        assert!(!s.apply(Action::Row(99)));
        assert_eq!(s.region, 2);
    }

    #[test]
    fn capability_rows_toggle_both_ways() {
        let mut s = setup();
        s.step = Step::Capabilities;
        let before = s.caps[2];
        s.apply(Action::Row(2));
        assert_ne!(s.caps[2], before);
        s.apply(Action::Row(2));
        assert_eq!(s.caps[2], before);
    }

    #[test]
    fn nothing_touching_personal_data_is_on_by_default() {
        use crate::caps::Cap;
        // Privacy first: out of the box the OS may search only the documents
        // that ship with it. Reading mail, indexing files, transcribing audio,
        // writing to disk and talking to remote services are all deliberate.
        let g = setup().grants();
        assert!(
            g.allows(Cap::SearchQuery),
            "built-in docs should be usable immediately"
        );
        for cap in [
            Cap::EmailSearch,
            Cap::WorkspaceIndex,
            Cap::AudioTranscribe,
            Cap::SkillsSave,
            Cap::PortalSync,
        ] {
            assert!(!g.allows(cap), "{} must be off by default", cap.name());
        }
    }

    #[test]
    fn grants_match_cap_module() {
        use crate::caps::Cap;
        // Rows show plain labels now, so the old name-equality check is gone.
        // What still has to hold is that row i drives Cap::ALL[i] — a mismatch
        // would put the right switch against the wrong capability, which is a
        // consent bug, not a cosmetic one.
        assert_eq!(N_CAPS, Cap::ALL.len());
        let s = setup();
        let g = s.grants();
        for (i, cap) in Cap::ALL.iter().enumerate() {
            assert_eq!(
                s.caps[i],
                g.allows(*cap),
                "row {i} ({}) drives the wrong cap",
                crate::caps::Cap::ALL[i].label()
            );
        }
    }

    #[test]
    fn every_row_toggles_exactly_its_own_capability() {
        use crate::caps::Cap;
        // Stronger than the mapping check: flip one row and confirm only that
        // capability moved.
        for i in 0..N_CAPS {
            let mut s = setup();
            let before = s.grants();
            s.step = Step::Capabilities;
            s.apply(Action::Row(i));
            let after = s.grants();
            for (j, cap) in Cap::ALL.iter().enumerate() {
                if i == j {
                    assert_ne!(
                        before.allows(*cap),
                        after.allows(*cap),
                        "row {i} did not toggle"
                    );
                } else {
                    assert_eq!(
                        before.allows(*cap),
                        after.allows(*cap),
                        "row {i} also changed {}",
                        cap.name()
                    );
                }
            }
        }
    }

    #[test]
    fn rows_do_nothing_on_steps_without_rows() {
        let mut s = setup();
        s.step = Step::Welcome;
        assert!(!s.apply(Action::Row(0)));
    }

    #[test]
    fn held_button_advances_only_once() {
        let mut s = setup();
        s.push_zone(0, 0, 100, 100, Action::Continue);
        assert!(s.pointer(10, 10, 1), "press should act");
        assert_eq!(s.step, Step::Experience);
        // Still held: must not keep advancing.
        assert!(!s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Experience);
        // Release then press again.
        assert!(!s.pointer(10, 10, 0));
        assert!(s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Region);
    }

    #[test]
    fn hover_is_feedback_not_activation() {
        let mut s = setup();
        s.push_zone(0, 0, 100, 100, Action::Continue);
        assert!(
            s.pointer_hover(10, 10),
            "entering a target should repaint"
        );
        assert_eq!(s.step, Step::Welcome, "hover must not advance setup");
        assert_eq!(s.hovered, Some(Action::Continue));
        assert!(
            !s.pointer_hover(20, 20),
            "moving inside one target is stable"
        );
        assert!(
            s.pointer_hover(200, 200),
            "leaving a target should repaint"
        );
        assert_eq!(s.hovered, None);
    }

    #[test]
    fn a_step_change_clears_the_old_hover_target() {
        let mut s = setup();
        s.push_zone(0, 0, 100, 100, Action::Continue);
        s.pointer_hover(10, 10);
        assert!(s.apply(Action::Continue));
        assert_eq!(s.step, Step::Experience);
        assert_eq!(s.hovered, None);
    }

    /// Every step must be reachable by keyboard alone — a machine whose
    /// pointer never arrives has to be finishable anyway.
    ///
    /// The list is the whole journey on purpose: it caught the merge that put
    /// `Experience` in front of `Region` without anyone updating the path.
    #[test]
    fn enter_can_complete_the_default_path_without_a_pointer() {
        let mut s = setup();
        for expected in [
            Step::Experience,
            Step::Region,
            Step::Capabilities,
            Step::Skills,
            Step::Done,
            Step::Finished,
        ] {
            s.reset_zones();
            s.push_zone(0, 0, 100, 40, Action::Continue);
            s.focus = 0;
            assert!(s.key(Key::Enter));
            assert_eq!(s.step, expected);
        }
    }

    #[test]
    fn tab_moves_focus_and_enter_activates_the_selected_row() {
        let mut s = setup();
        s.step = Step::Region;
        s.push_zone(0, 0, 100, 40, Action::Continue);
        s.push_zone(0, 50, 100, 40, Action::Row(2));
        s.focus = 0;
        assert!(s.key(Key::Tab));
        assert_eq!(s.focus, 1);
        assert!(s.key(Key::Enter));
        assert_eq!(s.region, 2);
        assert!(s.keyboard_focus);
    }

    #[test]
    fn clicks_outside_any_zone_are_ignored() {
        let mut s = setup();
        s.push_zone(0, 0, 50, 50, Action::Continue);
        assert!(!s.pointer(400, 400, 1));
        assert_eq!(s.step, Step::Welcome);
    }

    #[test]
    fn zone_table_cannot_overflow() {
        let mut s = setup();
        for _ in 0..MAX_ZONES * 3 {
            s.push_zone(0, 0, 10, 10, Action::Continue);
        }
        assert_eq!(s.n_zones, MAX_ZONES);
    }

    #[test]
    fn copy_is_ascii_only() {
        let mut all: Vec<&str> = vec![
            "hello",
            "How should it feel?",
            "You choose. Grants stay off until you turn them on.",
            "Select Your Region",
            "This sets formatting defaults. It does not leave the machine.",
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
            "Consent switches. Defaults stay privacy-first.",
            "Default Skills",
            "Markdown playbooks the agent can load. Editable later.",
            "Live from the host bridge. Tap Continue when ready.",
            "skills.list from bridge.",
            crate::copy::setup_skills_builtin(),
            "You're all set.",
            "Continue",
            "Go Back",
            "Start",
        ];
        all.extend(REGIONS);
        for level in Level::ALL {
            all.push(level.label());
            all.push(level.detail());
            all.push(level.done_subtitle());
            for (n, b) in cap_rows(level) {
                all.push(n);
                all.push(b);
            }
        }
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII copy renders as '?': {s:?}"
            );
        }
    }

    #[test]
    fn step_copy_fits_the_content_column() {
        for level in Level::ALL {
            for (n, b) in cap_rows(level) {
                assert!(
                    BRAND_FACE.width(n, 0) < CONTENT_W - 90,
                    "cap name too wide: {n}"
                );
                assert!(
                    SMALL_FACE.width(b, 0) < CONTENT_W - 90,
                    "cap blurb too wide: {b}"
                );
            }
            assert!(
                BRAND_FACE.width(level.label(), 0) < CONTENT_W - 60,
                "level too wide: {}",
                level.label()
            );
        }
        for r in REGIONS {
            assert!(
                BRAND_FACE.width(r, 0) < CONTENT_W - 60,
                "region too wide: {r}"
            );
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Mirrors the constants in `draw_caps` / `row`.
    fn rows_bottom(n: usize) -> i32 {
        // header() returns 132 + 76; draw_caps then asks row_pitch how tall
        // each row may be, so the mirror has to ask the same question.
        let top = 132 + 76;
        top + (n as i32) * row_pitch(top, 768, n, true) - ROW_GAP
    }

    /// Mirrors `footer`: pushed below the content, clamped onto the screen.
    fn footer_y(h: i32, content_bottom: i32) -> i32 {
        (content_bottom + 24).min(h - footer_height(false) - EDGE_MARGIN).max(0)
    }

    /// Mirrors `footer`.
    fn footer_top(h: i32) -> i32 {
        h - 150
    }

    #[test]
    fn capability_rows_clear_the_footer_at_768() {
        // Overlapping rows and the Continue pill would misroute clicks — the
        // exact failure a previous review caught on a short framebuffer.
        let bottom = rows_bottom(N_CAPS);
        assert!(
            bottom < footer_top(768),
            "{} capability rows reach {bottom}px, footer starts at {}",
            N_CAPS,
            footer_top(768)
        );
    }

    #[test]
    fn back_link_sits_above_the_continue_button_on_a_short_screen() {
        // The way back is now the nav "Back" (`nav_back`), which no list can
        // push off the bottom — `footer_visibility_tests` walks every step and
        // proves it. What still has to hold on a short screen is the other
        // half: Continue clears the last capability row and stays on screen.
        let bottom = rows_bottom(N_CAPS);
        let continue_y = footer_y(768, bottom);
        assert!(continue_y >= bottom, "Continue overlaps the last row");
        assert!(continue_y + CTA_H <= 768, "Continue falls off a 768px screen");
    }

    #[test]
    fn skills_rows_also_clear_the_footer() {
        let bottom = rows_bottom(7); // draw_skills shows up to seven builtins
        assert!(
            bottom < footer_top(768),
            "7 skill rows reach {bottom}px, footer starts at {}",
            footer_top(768)
        );
    }

    #[test]
    fn the_capability_list_is_at_its_layout_limit() {
        // Seven single-line rows fit above the footer at 768px (Send mail).
        // An eighth would need scroll/paginate — do not shrink rows to squeeze.
        assert_eq!(N_CAPS, 7, "update this guard when Caps grow again");
        assert!(
            rows_bottom(N_CAPS) < footer_top(768),
            "{} capability rows already collide with the footer",
            N_CAPS
        );
        assert!(
            rows_bottom(8) >= footer_top(768),
            "layout gained room for an 8th row — update this guard deliberately"
        );
    }

    #[test]
    fn every_capability_row_is_reachable_by_click() {
        let mut s = Setup::new();
        s.step = Step::Capabilities;
        for i in 0..N_CAPS {
            let before = s.caps[i];
            assert!(s.apply(Action::Row(i)), "row {i} did nothing");
            assert_ne!(s.caps[i], before, "row {i} did not toggle");
        }
    }

    #[test]
    fn capability_names_and_blurbs_fit_the_column() {
        for level in Level::ALL {
            for (name, blurb) in cap_rows(level) {
                // Label and consequence share one baseline, so they must fit
                // side by side without reaching the switch.
                let used = BRAND_FACE.width(name, 0) + 12 + SMALL_FACE.width(blurb, 0);
                assert!(
                    used < CONTENT_W - 80,
                    "row {name:?} ({}) overruns the switch: {used}px",
                    level.label()
                );
            }
        }
    }
}

#[cfg(test)]
mod footer_visibility_tests {
    use super::*;
    use crate::fb::Surface;

    /// Draw every step of the journey and collect the zones it registered.
    pub(super) fn walk(w: i32, h: i32) -> Vec<(Step, Vec<Zone>)> {
        let mut s = Setup::new();
        let mut buf = vec![0u32; (w * h) as usize];
        let surface = unsafe { Surface::in_memory(buf.as_mut_ptr(), w as usize, h as usize) };
        let mail = crate::mcp::MailPeek::empty(crate::mcp::BridgeStatus::Offline);
        let skills = crate::skills::SkillPeek::from_builtin();
        let mut out = Vec::new();
        for _ in 0..8 {
            if s.step == Step::Finished {
                break;
            }
            s.draw(&surface, &mail, &skills);
            out.push((s.step, s.zones[..s.n_zones].to_vec()));
            s.apply(Action::Continue);
        }
        out
    }

    #[test]
    fn every_clickable_zone_lands_on_screen() {
        // An off-screen zone is the worst kind of broken: the code believes
        // the affordance exists and the person cannot see or reach it.
        for (w, h) in [(800, 600), (1024, 768), (1280, 800)] {
            for (step, zones) in walk(w, h) {
                for z in zones {
                    assert!(
                        z.y >= 0 && z.y + z.h <= h,
                        "{step:?} at {w}x{h}: zone spans {}..{} outside 0..{h}",
                        z.y,
                        z.y + z.h
                    );
                }
            }
        }
    }

    #[test]
    fn the_longest_step_still_offers_a_visible_way_back() {
        // Capabilities has the most rows, so it is the step whose own footer
        // pushed the back link out of sight.
        for (w, h) in [(800, 600), (1024, 768), (1280, 800)] {
            let (_, zones) = walk(w, h)
                .into_iter()
                .find(|(step, _)| *step == Step::Capabilities)
                .expect("capabilities is in the journey");
            assert!(
                zones
                    .iter()
                    .any(|z| z.action == Action::Back && z.y >= 0 && z.y + z.h <= h),
                "no visible way back from Capabilities at {w}x{h}"
            );
        }
    }
}

#[cfg(test)]
mod no_overlap_tests {
    use super::*;

    fn overlaps(a: &Zone, b: &Zone) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn no_two_clickable_zones_sit_on_top_of_each_other() {
        // Continue was drawn over the last capability row: the click still
        // worked, but the row underneath it was unreadable and its own zone
        // was unreachable.
        for (w, h) in [(800, 600), (1024, 768), (1280, 800)] {
            for (step, zones) in super::footer_visibility_tests::walk(w, h) {
                for i in 0..zones.len() {
                    for j in i + 1..zones.len() {
                        assert!(
                            !overlaps(&zones[i], &zones[j]),
                            "{step:?} at {w}x{h}: {:?} overlaps {:?}",
                            zones[i].action,
                            zones[j].action
                        );
                    }
                }
            }
        }
    }
}

/// The "which input is missing" note.
///
/// It used to be painted onto Home during boot, where setup drew over it a
/// moment later — so on a machine that cannot be driven at all, the one
/// sentence explaining why nothing responds was never readable. Welcome is
/// the screen such a machine is stuck on, so Welcome carries it.
#[cfg(test)]
mod input_note_tests {
    use super::*;
    use crate::fb::Surface;
    use crate::inputdiag::Inputs;
    use crate::pci::UsbSurvey;

    /// Every note the diagnostic can produce, so a new one cannot quietly
    /// arrive too wide for the screen.
    fn all_notes() -> Vec<&'static str> {
        let mut out = Vec::new();
        for ps2_controller in [false, true] {
            for ps2 in [false, true] {
                for mouse in [false, true] {
                    for usb_kbd in [false, true] {
                        for tablet in [false, true] {
                            for usb in [UsbSurvey::default(), unsupported_usb()] {
                                let i = Inputs {
                                    ps2_controller,
                                    ps2_keyboard: ps2,
                                    ps2_mouse: mouse,
                                    usb_keyboard: usb_kbd,
                                    usb_tablet: tablet,
                                    usb,
                                };
                                if let Some(n) = i.note() {
                                    if !out.contains(&n) {
                                        out.push(n);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(!out.is_empty(), "inputdiag produced no notes at all");
        out
    }

    fn unsupported_usb() -> UsbSurvey {
        let mut u = UsbSurvey::default();
        u.xhci = 1;
        u
    }

    /// Non-background pixels in the horizontal band `ys`.
    fn ink_in_band(w: i32, h: i32, ys: core::ops::Range<i32>, note: Option<&'static str>) -> usize {
        let mut s = Setup::new();
        s.set_note(note);
        let mut buf = vec![0u32; (w * h) as usize];
        let surface = unsafe { Surface::in_memory(buf.as_mut_ptr(), w as usize, h as usize) };
        let mail = crate::mcp::MailPeek::empty(crate::mcp::BridgeStatus::Offline);
        let skills = crate::skills::SkillPeek::from_builtin();
        s.draw(&surface, &mail, &skills);
        assert_eq!(s.step, Step::Welcome, "this test is about the first screen");
        buf[(ys.start * w) as usize..(ys.end * w) as usize]
            .iter()
            .filter(|&&p| p & 0x00FF_FFFF != theme::BG)
            .count()
    }

    #[test]
    fn welcome_says_which_input_is_missing() {
        for (w, h) in [(800, 600), (1024, 768), (1280, 800)] {
            let band = 20..52;
            assert_eq!(
                ink_in_band(w, h, band.clone(), None),
                0,
                "at {w}x{h} the top bar must stay empty when every input came up"
            );
            for note in all_notes() {
                assert!(
                    ink_in_band(w, h, band.clone(), Some(note)) > 0,
                    "at {w}x{h} the note {note:?} was not drawn"
                );
            }
        }
    }

    #[test]
    fn no_note_is_too_wide_for_the_narrowest_screen() {
        // Centred text that overruns the surface is clipped at both ends, which
        // reads as a different sentence rather than a truncated one.
        const NARROWEST: i32 = 800;
        for note in all_notes() {
            let tw = SMALL_FACE.width(note, 0);
            assert!(
                tw <= NARROWEST - 32,
                "{note:?} is {tw}px wide, past the {NARROWEST}px screen it has to fit"
            );
        }
    }

    #[test]
    fn the_note_clears_the_continue_button() {
        for (w, h) in [(800, 600), (1024, 768), (1280, 800)] {
            let mut s = Setup::new();
            s.set_note(Some(all_notes()[0]));
            let mut buf = vec![0u32; (w * h) as usize];
            let surface = unsafe { Surface::in_memory(buf.as_mut_ptr(), w as usize, h as usize) };
            let mail = crate::mcp::MailPeek::empty(crate::mcp::BridgeStatus::Offline);
            let skills = crate::skills::SkillPeek::from_builtin();
            s.draw(&surface, &mail, &skills);
            for z in &s.zones[..s.n_zones] {
                assert!(
                    z.y > 52,
                    "at {w}x{h} the {:?} zone starts at {} - the note is drawn there",
                    z.action,
                    z.y
                );
            }
        }
    }

    #[test]
    fn restarting_setup_keeps_the_note() {
        // The pointer did not come back while the person clicked "Ready".
        let note = all_notes()[0];
        let s = Setup::restart(Level::Advanced, Some(note));
        assert_eq!(s.note, Some(note));
        assert_eq!(s.level, Level::Advanced, "restart still keeps the level");
    }
}
