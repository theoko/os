//! First-boot setup journey, in the shape of the macOS Setup Assistant.
//!
//! One decision per screen, centred on a white page, with a single primary
//! action and a quiet way back. The steps mirror what this OS actually has to
//! establish before the agent can do anything: where it is, whether the host
//! bridge is reachable, which capabilities are granted, and which skills load.
//!
//! Drawing records its own hit zones, so `click()` needs no separate layout
//! table to drift out of sync.

use crate::caps::Caps;
use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, HERO_FACE, SMALL_FACE, TITLE_FACE};
use crate::mcp::{BridgeStatus, MailPeek};
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

#[derive(Clone, Copy)]
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
    Region,
    Bridge,
    Capabilities,
    Skills,
    Done,
    /// Setup finished; the home screen takes over.
    Finished,
}

pub const REGIONS: [&str; 4] = ["United States", "United Kingdom", "Greece", "Japan"];

/// Capabilities the agent may be granted up front. Mirrors [`Cap`] / bridge tools.
pub const CAPS: [(&str, &str); 5] = [
    ("email.search", "Read the inbox through the host bridge"),
    ("search.query", "Query the built-in knowledge corpus"),
    ("skills.save", "Write new skill playbooks to disk"),
    ("workspace.index", "Search your own files on this machine"),
    ("audio.transcribe", "Transcribe recordings and index what was said"),
];

const MAX_ZONES: usize = 12;

pub struct Setup {
    pub step: Step,
    pub region: usize,
    pub caps: [bool; CAPS.len()],
    zones: [Zone; MAX_ZONES],
    n_zones: usize,
    /// Edge detection: a held button must not advance every frame.
    was_down: bool,
}

impl Setup {
    pub const fn new() -> Self {
        Self {
            step: Step::Welcome,
            region: 0,
            // Read-only tools on; anything that writes to disk or reaches
            // personal files is opt-in, matching the "no ambient root" rule.
            caps: [true, true, false, false, false],
            zones: [Zone {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                action: Action::Continue,
            }; MAX_ZONES],
            n_zones: 0,
            was_down: false,
        }
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
        match self.hit(x, y) {
            Some(action) => self.apply(action),
            None => false,
        }
    }

    /// Apply an action. Returns true when the screen changed.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Continue => {
                self.step = match self.step {
                    Step::Welcome => Step::Region,
                    Step::Region => Step::Bridge,
                    Step::Bridge => Step::Capabilities,
                    Step::Capabilities => Step::Skills,
                    Step::Skills => Step::Done,
                    Step::Done => Step::Finished,
                    Step::Finished => Step::Finished,
                };
                true
            }
            Action::Back => {
                self.step = match self.step {
                    Step::Welcome | Step::Region => Step::Welcome,
                    Step::Bridge => Step::Region,
                    Step::Capabilities => Step::Bridge,
                    Step::Skills => Step::Capabilities,
                    Step::Done => Step::Skills,
                    Step::Finished => Step::Finished,
                };
                true
            }
            Action::Row(i) => match self.step {
                Step::Region => {
                    if i < REGIONS.len() {
                        self.region = i;
                        return true;
                    }
                    false
                }
                Step::Capabilities => {
                    if i < CAPS.len() {
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
    pub fn draw(&mut self, fb: &Surface, mail: &MailPeek, skills: &SkillPeek) {
        let w = fb.width() as i32;
        let h = fb.height() as i32;
        fb.fill(theme::BG);
        self.reset_zones();

        match self.step {
            Step::Welcome => self.draw_welcome(fb, w, h),
            Step::Region => self.draw_region(fb, w, h),
            Step::Bridge => self.draw_bridge(fb, w, h, mail),
            Step::Capabilities => self.draw_caps(fb, w, h),
            Step::Skills => self.draw_skills(fb, w, h, skills),
            Step::Done => self.draw_done(fb, w, h),
            Step::Finished => {}
        }
    }

    fn draw_welcome(&mut self, fb: &Surface, w: i32, h: i32) {
        // Apple opens on a single word and nothing else.
        let track = font::tracking_pct(HERO_FACE.px, -30);
        let cy = h / 2 - 40;
        fb.draw_text_centered(w / 2, cy, "hello", &HERO_FACE, track, theme::INK);
        self.primary(fb, w, cy + 90, "Continue");
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
            self.row(fb, w, y, name, None, i == sel, false, Action::Row(i));
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y, true);
    }

    fn draw_bridge(&mut self, fb: &Surface, w: i32, h: i32, mail: &MailPeek) {
        let online = matches!(mail.status, BridgeStatus::Online);
        let top = self.header(
            fb,
            w,
            h,
            "Connect the Bridge",
            "Connectors run on the host, never in the kernel.",
        );
        let (label, detail, tint) = if online {
            ("Host bridge", "Connected on COM2", theme::ONLINE)
        } else {
            ("Host bridge", "Offline - start it with 'make bridge-run'", theme::OFFLINE)
        };
        self.status_card(fb, w, top, label, detail, tint);
        self.footer(fb, w, h, top + ROW_H + 8, true);
    }

    fn draw_caps(&mut self, fb: &Surface, w: i32, h: i32) {
        let top = self.header(
            fb,
            w,
            h,
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
        );
        let mut y = top;
        for (i, (name, blurb)) in CAPS.iter().enumerate() {
            let on = self.caps[i];
            self.row(fb, w, y, name, Some(blurb), on, true, Action::Row(i));
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y, true);
    }

    fn draw_skills(&mut self, fb: &Surface, w: i32, h: i32, skills: &SkillPeek) {
        let top = self.header(
            fb,
            w,
            h,
            "Default Skills",
            "Markdown playbooks the agent can load. Editable later.",
        );
        let mut y = top;
        for i in 0..skills.count.min(4) {
            self.row(fb, w, y, skills.name_at(i), None, true, false, Action::Row(i));
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y, true);
    }

    fn draw_done(&mut self, fb: &Surface, w: i32, h: i32) {
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        let cy = h / 2 - 40;
        fb.draw_text_centered(w / 2, cy, "You're all set.", &TITLE_FACE, track, theme::INK);
        fb.draw_text_centered(
            w / 2,
            cy + 40,
            "Capabilities granted. Skills loaded.",
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
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        let y = 132;
        fb.draw_text_centered(w / 2, y, title, &TITLE_FACE, track, theme::INK);
        fb.draw_text_centered(w / 2, y + 36, sub, &BODY_FACE, 0, theme::MUTED);
        y + 76
    }

    /// A selectable row. `toggle` draws a switch instead of a checkmark.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        fb: &Surface,
        w: i32,
        y: i32,
        title: &str,
        blurb: Option<&str>,
        on: bool,
        toggle: bool,
        action: Action,
    ) {
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;

        // Selected rows get an accent hairline; the rest a neutral one.
        let border = if on && !toggle { theme::ACCENT } else { theme::CARD_BORDER };
        fb.fill_round_rect(x, y, cw, ROW_H, 10, border);
        let inner = if on && !toggle { theme::TINT_BG } else { theme::BG };
        fb.fill_round_rect(x + 1, y + 1, cw - 2, ROW_H - 2, 9, inner);

        let pad = 18;
        let has_blurb = blurb.is_some();
        let title_base = if has_blurb {
            y + 24
        } else {
            y + ROW_H / 2 + BRAND_FACE.px / 3
        };
        fb.draw_text(x + pad, title_base, title, &BRAND_FACE, 0, theme::INK);
        if let Some(b) = blurb {
            fb.draw_text(x + pad, title_base + 20, b, &SMALL_FACE, 0, theme::MUTED);
        }

        if toggle {
            // Pill switch, filled when granted.
            let tw = 40;
            let th = 22;
            let tx = x + cw - pad - tw;
            let ty = y + (ROW_H - th) / 2;
            let track_col = if on { theme::ACCENT } else { theme::RULE };
            fb.fill_round_rect(tx, ty, tw, th, th / 2, track_col);
            let knob = th - 6;
            let kx = if on { tx + tw - knob - 3 } else { tx + 3 };
            fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::BG);
        } else if on {
            // Selection dot.
            let d = 10;
            fb.fill_round_rect(
                x + cw - pad - d,
                y + (ROW_H - d) / 2,
                d,
                d,
                d / 2,
                theme::ACCENT,
            );
        }

        self.push_zone(x, y, cw, ROW_H, action);
    }

    fn status_card(&mut self, fb: &Surface, w: i32, y: i32, label: &str, detail: &str, tint: u32) {
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;
        fb.fill_round_rect(x, y, cw, ROW_H + 8, 10, theme::CARD_BORDER);
        fb.fill_round_rect(x + 1, y + 1, cw - 2, ROW_H + 6, 9, theme::BG);
        let pad = 18;
        let d = 9;
        fb.fill_round_rect(x + pad, y + (ROW_H + 8 - d) / 2, d, d, d / 2, tint);
        fb.draw_text(x + pad + d + 12, y + 26, label, &BRAND_FACE, 0, theme::INK);
        fb.draw_text(x + pad + d + 12, y + 46, detail, &SMALL_FACE, 0, theme::MUTED);
    }

    /// Primary pill, centred, registering a Continue zone.
    fn primary(&mut self, fb: &Surface, w: i32, y: i32, label: &str) {
        let pad = 40;
        let bw = (BTN_FACE.width(label, 0) + pad * 2).max(180);
        let x = w / 2 - bw / 2;
        fb.fill_round_rect(x, y, bw, CTA_H, CTA_H / 2, theme::ACCENT);
        let base = y + (CTA_H - BTN_FACE.px) / 2 + BTN_FACE.baseline() - 2;
        fb.draw_text_centered(w / 2, base, label, &BTN_FACE, 0, theme::BG);
        self.push_zone(x, y, bw, CTA_H, Action::Continue);
    }

    fn back_link(&mut self, fb: &Surface, w: i32, y: i32) {
        let label = "Go Back";
        let tw = BTN_FACE.width(label, 0);
        let x = w / 2 - tw / 2;
        fb.draw_text(x, y + BTN_FACE.baseline(), label, &BTN_FACE, 0, theme::ACCENT);
        // Generous target: the text alone is a 15px-tall sliver.
        self.push_zone(x - 12, y - 8, tw + 24, BTN_FACE.px + 20, Action::Back);
    }

    /// `content_bottom` = y just below the last row/card: on short
    /// framebuffers the pill moves down rather than overlapping the rows
    /// (zones are hit first-match, so an overlap misroutes clicks).
    fn footer(&mut self, fb: &Surface, w: i32, h: i32, content_bottom: i32, back: bool) {
        let y = (h - 150).max(content_bottom + 24);
        self.primary(fb, w, y, "Continue");
        if back {
            self.back_link(fb, w, y + CTA_H + 26);
        }
    }
}

const CONTENT_W: i32 = 460;
const ROW_H: i32 = 58;
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
            Step::Region,
            Step::Bridge,
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
        assert_eq!(s.step, Step::Bridge);
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
    fn skills_write_is_off_by_default() {
        // "No ambient root" - granting disk writes must be a deliberate act.
        let s = setup();
        let idx = CAPS.iter().position(|(n, _)| *n == "skills.save").unwrap();
        assert!(!s.caps[idx]);
    }

    #[test]
    fn grants_match_cap_module() {
        use crate::caps::Cap;
        let s = setup();
        let g = s.grants();
        for (i, cap) in Cap::ALL.iter().enumerate() {
            assert_eq!(CAPS[i].0, cap.name());
            assert_eq!(s.caps[i], g.allows(*cap));
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
        assert_eq!(s.step, Step::Region);
        // Still held: must not keep advancing.
        assert!(!s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Region);
        // Release then press again.
        assert!(!s.pointer(10, 10, 0));
        assert!(s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Bridge);
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
            "Select Your Region",
            "This sets formatting defaults. It does not leave the machine.",
            "Connect the Bridge",
            "Connectors run on the host, never in the kernel.",
            "Offline - start it with 'make bridge-run'",
            "Connected on COM2",
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
            "Default Skills",
            "Markdown playbooks the agent can load. Editable later.",
            "You're all set.",
            "Capabilities granted. Skills loaded.",
            "Continue",
            "Go Back",
            "Start",
            "Host bridge",
        ];
        all.extend(REGIONS);
        for (n, b) in CAPS {
            all.push(n);
            all.push(b);
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
        for (n, b) in CAPS {
            assert!(BRAND_FACE.width(n, 0) < CONTENT_W - 90, "cap name too wide: {n}");
            assert!(SMALL_FACE.width(b, 0) < CONTENT_W - 90, "cap blurb too wide: {b}");
        }
        for r in REGIONS {
            assert!(BRAND_FACE.width(r, 0) < CONTENT_W - 60, "region too wide: {r}");
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Mirrors the constants in `draw_caps` / `row`.
    fn rows_bottom(n: usize) -> i32 {
        // header() returns 132 + 76; rows are ROW_H apart with an 8px gap.
        (132 + 76) + (n as i32) * (ROW_H + 8) - 8
    }

    /// Mirrors `footer`.
    fn footer_top(h: i32) -> i32 {
        h - 150
    }

    #[test]
    fn capability_rows_clear_the_footer_at_768() {
        // Overlapping rows and the Continue pill would misroute clicks — the
        // exact failure a previous review caught on a short framebuffer.
        let bottom = rows_bottom(CAPS.len());
        assert!(
            bottom < footer_top(768),
            "{} capability rows reach {bottom}px, footer starts at {}",
            CAPS.len(),
            footer_top(768)
        );
    }

    #[test]
    fn skills_rows_also_clear_the_footer() {
        let bottom = rows_bottom(4); // draw_skills caps the list at 4
        assert!(bottom < footer_top(768));
    }

    #[test]
    fn there_is_headroom_for_one_more_capability() {
        // Capabilities have grown 3 -> 5 in this session; make the next
        // addition fail loudly here rather than silently on screen.
        assert!(
            rows_bottom(CAPS.len() + 1) < footer_top(768),
            "adding another capability would collide with the footer"
        );
    }

    #[test]
    fn every_capability_row_is_reachable_by_click() {
        let mut s = Setup::new();
        s.step = Step::Capabilities;
        for i in 0..CAPS.len() {
            let before = s.caps[i];
            assert!(s.apply(Action::Row(i)), "row {i} did nothing");
            assert_ne!(s.caps[i], before, "row {i} did not toggle");
        }
    }

    #[test]
    fn capability_names_and_blurbs_fit_the_column() {
        for (name, blurb) in CAPS {
            assert!(BRAND_FACE.width(name, 0) < CONTENT_W - 76, "name hits the switch: {name}");
            assert!(SMALL_FACE.width(blurb, 0) < CONTENT_W - 76, "blurb hits the switch: {blurb}");
        }
    }
}
