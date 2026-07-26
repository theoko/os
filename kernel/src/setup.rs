//! First-boot setup journey, in the shape of the macOS Setup Assistant.
//!
//! One decision per screen, centred on a white page, with a single primary
//! action and a quiet way back. The steps mirror what this OS actually has to
//! establish before the agent can do anything: whether the host bridge is
//! reachable, which capabilities are granted, and which skills load.
//!
//! Drawing records its own hit zones, so `click()` needs no separate layout
//! table to drift out of sync.

use crate::caps::{Cap, Caps};
use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, HERO_FACE, SMALL_FACE, TITLE_FACE};
use crate::mcp::BridgeStatus;
use crate::skills::SkillPeek;
use crate::ui::{self, theme};

/// Where a click landed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Continue,
    Back,
    /// Capability row toggle (setup Skills rows are paint-only).
    Row(usize),
}

#[derive(Clone, Copy)]
struct Zone {
    rect: ui::Rect,
    action: Action,
}

/// The journey, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Welcome,
    Bridge,
    Capabilities,
    Skills,
    Done,
    /// Setup finished; the home screen takes over.
    Finished,
}

/// Blurbs for each [`Cap::ALL`] row (tool names come from [`Cap::name`]).
pub const CAP_BLURBS: [&str; 5] = [
    "Read the inbox through the host bridge",
    "Query the built-in knowledge corpus",
    "Write new skill playbooks to disk",
    "Search your own files on this machine",
    "Transcribe recordings and index what was said",
];

const MAX_ZONES: usize = 12;

pub struct Setup {
    pub step: Step,
    pub caps: Caps,
    zones: [Zone; MAX_ZONES],
    n_zones: usize,
    /// Edge detection: a held button must not advance every frame.
    was_down: bool,
}

impl Setup {
    pub const fn new() -> Self {
        Self {
            step: Step::Welcome,
            // Read-only tools on; anything that writes to disk or reaches
            // personal files is opt-in — see Caps::default_grants.
            caps: Caps::default_grants(),
            zones: [Zone {
                rect: ui::Rect::new(0, 0, 0, 0),
                action: Action::Continue,
            }; MAX_ZONES],
            n_zones: 0,
            was_down: false,
        }
    }

    /// Grant set chosen on the Capabilities step.
    pub fn grants(&self) -> Caps {
        self.caps
    }

    pub fn is_finished(&self) -> bool {
        self.step == Step::Finished
    }

    fn reset_zones(&mut self) {
        self.n_zones = 0;
    }

    fn push_zone(&mut self, x: i32, y: i32, w: i32, h: i32, action: Action) {
        if self.n_zones < MAX_ZONES {
            self.zones[self.n_zones] = Zone {
                rect: ui::Rect::new(x, y, w, h),
                action,
            };
            self.n_zones += 1;
        }
    }

    fn hit(&self, px: i32, py: i32) -> Option<Action> {
        self.zones[..self.n_zones]
            .iter()
            .find(|z| z.rect.contains(px, py))
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
                    Step::Welcome => Step::Bridge,
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
                    Step::Welcome => Step::Welcome,
                    Step::Bridge => Step::Welcome,
                    Step::Capabilities => Step::Bridge,
                    Step::Skills => Step::Capabilities,
                    Step::Done => Step::Skills,
                    Step::Finished => Step::Finished,
                };
                true
            }
            Action::Row(i) => match self.step {
                Step::Capabilities => {
                    if let Some(&cap) = Cap::ALL.get(i) {
                        self.caps.set(cap, !self.caps.allows(cap));
                        return true;
                    }
                    false
                }
                _ => false,
            },
        }
    }

    /// Paint the current step. Records hit zones as a side effect.
    pub fn draw(&mut self, fb: &Surface, status: BridgeStatus, skills: &SkillPeek) {
        let w = fb.width() as i32;
        let h = fb.height() as i32;
        fb.fill(theme::BG);
        self.reset_zones();

        match self.step {
            Step::Welcome => self.draw_welcome(fb, w, h),
            Step::Bridge => self.draw_bridge(fb, w, h, status),
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

    fn draw_bridge(&mut self, fb: &Surface, w: i32, h: i32, status: BridgeStatus) {
        let top = self.header(
            fb,
            w,
            "Connect the Bridge",
            "Connectors run on the host, never in the kernel.",
        );
        self.status_card(fb, w, top, status);
        self.footer(fb, w, h, top + ROW_H + 8);
    }

    fn draw_caps(&mut self, fb: &Surface, w: i32, h: i32) {
        let top = self.header(
            fb,
            w,
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
        );
        let mut y = top;
        for i in 0..Cap::ALL.len() {
            self.cap_row(fb, w, y, i);
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y);
    }

    fn draw_skills(&mut self, fb: &Surface, w: i32, h: i32, skills: &SkillPeek) {
        let top = self.header(
            fb,
            w,
            "Default Skills",
            if skills.from_bridge {
                "Live from the host bridge. Tap Continue when ready."
            } else {
                "Markdown playbooks the agent can load. Editable later."
            },
        );
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;
        let mut y = top;
        for i in 0..skills.count.min(4) {
            let desc = skills.desc_at(i);
            let sub = if desc.is_empty() {
                if skills.from_bridge {
                    "From host bridge"
                } else {
                    "Shipped with the ISO"
                }
            } else {
                desc
            };
            ui::draw_titled_row(fb, ui::Rect::new(x, y, cw, ROW_H), skills.name_at(i), sub);
            y += ROW_H + 8;
        }
        self.footer(fb, w, h, y);
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
    fn header(&mut self, fb: &Surface, w: i32, title: &str, sub: &str) -> i32 {
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        let y = 132;
        fb.draw_text_centered(w / 2, y, title, &TITLE_FACE, track, theme::INK);
        fb.draw_text_centered(w / 2, y + 36, sub, &BODY_FACE, 0, theme::MUTED);
        y + 76
    }

    /// Capability toggle at index `i`: titled row + switch + hit zone.
    fn cap_row(&mut self, fb: &Surface, w: i32, y: i32, i: usize) {
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;
        ui::draw_titled_row(
            fb,
            ui::Rect::new(x, y, cw, ROW_H),
            Cap::ALL[i].name(),
            CAP_BLURBS[i],
        );
        let pad = 18;
        ui::draw_switch(
            fb,
            x + cw - pad - ui::SWITCH_W,
            y + (ROW_H - ui::SWITCH_H) / 2,
            self.caps.allows(Cap::ALL[i]),
        );
        self.push_zone(x, y, cw, ROW_H, Action::Row(i));
    }

    fn status_card(&mut self, fb: &Surface, w: i32, y: i32, status: BridgeStatus) {
        let (detail, tint) = match status {
            BridgeStatus::Online => ("Connected on COM2", theme::ONLINE),
            BridgeStatus::Offline => (crate::mcp::BRIDGE_OFFLINE_HINT, theme::OFFLINE),
        };
        let cw = CONTENT_W.min(w - 80);
        let x = (w - cw) / 2;
        ui::outlined_round_rect(fb, x, y, cw, ROW_H + 8, 10);
        let pad = 18;
        let d = 9;
        fb.fill_round_rect(x + pad, y + (ROW_H + 8 - d) / 2, d, d, d / 2, tint);
        fb.draw_text(x + pad + d + 12, y + 26, "Host bridge", &BRAND_FACE, 0, theme::INK);
        fb.draw_text(x + pad + d + 12, y + 46, detail, &SMALL_FACE, 0, theme::MUTED);
    }

    /// Primary pill, centred, registering a Continue zone.
    fn primary(&mut self, fb: &Surface, w: i32, y: i32, label: &str) {
        let pad = 40;
        let bw = (BTN_FACE.width(label, 0) + pad * 2).max(180);
        let x = w / 2 - bw / 2;
        fb.fill_round_rect(x, y, bw, CTA_H, CTA_H / 2, theme::ACCENT);
        let base = y + (CTA_H - BTN_FACE.px) / 2 + BTN_FACE.baseline() - 2;
        fb.draw_text_centered(w / 2, base, label, &BTN_FACE, 0, theme::SURFACE);
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
    fn footer(&mut self, fb: &Surface, w: i32, h: i32, content_bottom: i32) {
        let y = (h - 150).max(content_bottom + 24);
        self.primary(fb, w, y, "Continue");
        self.back_link(fb, w, y + CTA_H + 26);
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
        for _ in 0..3 {
            s.apply(Action::Continue);
        }
        assert_eq!(s.step, Step::Skills);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Capabilities);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Bridge);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Welcome);
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
    fn capability_rows_toggle_both_ways() {
        let mut s = setup();
        s.step = Step::Capabilities;
        let cap = Cap::ALL[2];
        let before = s.caps.allows(cap);
        s.apply(Action::Row(2));
        assert_ne!(s.caps.allows(cap), before);
        s.apply(Action::Row(2));
        assert_eq!(s.caps.allows(cap), before);
    }

    #[test]
    fn skills_write_is_off_by_default() {
        // "No ambient root" - granting disk writes must be a deliberate act.
        let s = setup();
        assert!(!s.caps.allows(Cap::SkillsSave));
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
        assert_eq!(s.step, Step::Bridge);
        // Still held: must not keep advancing.
        assert!(!s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Bridge);
        // Release then press again.
        assert!(!s.pointer(10, 10, 0));
        assert!(s.pointer(10, 10, 1));
        assert_eq!(s.step, Step::Capabilities);
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
            "Connect the Bridge",
            "Connectors run on the host, never in the kernel.",
            crate::mcp::BRIDGE_OFFLINE_HINT,
            "Connected on COM2",
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
            "Default Skills",
            "Markdown playbooks the agent can load. Editable later.",
            "Live from the host bridge. Tap Continue when ready.",
            "From host bridge",
            "Shipped with the ISO",
            "You're all set.",
            "Capabilities granted. Skills loaded.",
            "Continue",
            "Go Back",
            "Start",
            "Host bridge",
        ];
        for cap in Cap::ALL {
            all.push(cap.name());
        }
        all.extend_from_slice(&CAP_BLURBS);
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII copy renders as '?': {s:?}"
            );
        }
    }

    #[test]
    fn step_copy_fits_the_content_column() {
        for (i, cap) in Cap::ALL.iter().enumerate() {
            let n = cap.name();
            let b = CAP_BLURBS[i];
            assert!(BRAND_FACE.width(n, 0) < CONTENT_W - 90, "cap name too wide: {n}");
            assert!(SMALL_FACE.width(b, 0) < CONTENT_W - 90, "cap blurb too wide: {b}");
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
        let bottom = rows_bottom(Cap::ALL.len());
        assert!(
            bottom < footer_top(768),
            "{} capability rows reach {bottom}px, footer starts at {}",
            Cap::ALL.len(),
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
            rows_bottom(Cap::ALL.len() + 1) < footer_top(768),
            "adding another capability would collide with the footer"
        );
    }

    #[test]
    fn every_capability_row_is_reachable_by_click() {
        let mut s = Setup::new();
        s.step = Step::Capabilities;
        for i in 0..Cap::ALL.len() {
            let cap = Cap::ALL[i];
            let before = s.caps.allows(cap);
            assert!(s.apply(Action::Row(i)), "row {i} did nothing");
            assert_ne!(s.caps.allows(cap), before, "row {i} did not toggle");
        }
    }

    #[test]
    fn capability_names_and_blurbs_fit_the_column() {
        for (i, cap) in Cap::ALL.iter().enumerate() {
            let name = cap.name();
            let blurb = CAP_BLURBS[i];
            assert!(BRAND_FACE.width(name, 0) < CONTENT_W - 76, "name hits the switch: {name}");
            assert!(SMALL_FACE.width(blurb, 0) < CONTENT_W - 76, "blurb hits the switch: {blurb}");
        }
    }
}
