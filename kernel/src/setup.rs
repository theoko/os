//! First-boot setup journey, in the shape of the macOS Setup Assistant.
//!
//! One decision per screen, centred on a light page, with a single primary
//! action and a quiet way back. The steps mirror what this OS actually has to
//! establish before the agent can do anything: whether the host bridge is
//! reachable, which capabilities are granted, and which skills load.
//!
//! Drawing records its own hit zones, so `click()` needs no separate layout
//! table to drift out of sync.

use crate::caps::{Cap, Caps};
use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, HERO_FACE, SMALL_FACE, TITLE_FACE};
use crate::skills::SkillPeek;
use crate::ui::{self, theme};

/// Where a click landed.
#[derive(Clone, Copy)]
enum Action {
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

const MAX_ZONES: usize = 12;

pub struct Setup {
    pub step: Step,
    pub caps: Caps,
    zones: [Zone; MAX_ZONES],
    n_zones: usize,
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
        }
    }

    pub fn is_finished(&self) -> bool {
        self.step == Step::Finished
    }

    fn reset_zones(&mut self) {
        self.n_zones = 0;
    }

    fn push_zone(&mut self, rect: ui::Rect, action: Action) {
        if self.n_zones < MAX_ZONES {
            self.zones[self.n_zones] = Zone { rect, action };
            self.n_zones += 1;
        }
    }

    /// Apply a click at `(x, y)`. Returns true when the screen needs redrawing.
    ///
    /// Rising-edge filtering lives in the main loop (`Mouse::take_click_edge`)
    /// so setup does not keep a parallel button latch.
    pub fn click(&mut self, x: i32, y: i32) -> bool {
        match self.zones[..self.n_zones]
            .iter()
            .find(|z| z.rect.contains(x, y))
            .map(|z| z.action)
        {
            Some(action) => self.apply(action),
            None => false,
        }
    }

    /// Apply an action. Returns true when the screen changed.
    fn apply(&mut self, action: Action) -> bool {
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
                    let before = self.caps;
                    self.caps.toggle(i);
                    self.caps != before
                }
                _ => false,
            },
        }
    }

    /// Paint the current step. Records hit zones as a side effect.
    pub fn draw(&mut self, fb: &Surface, online: bool, skills: &SkillPeek) {
        fb.fill();
        self.reset_zones();

        match self.step {
            Step::Welcome => self.draw_welcome(fb),
            Step::Bridge => self.draw_bridge(fb, online),
            Step::Capabilities => self.draw_caps(fb),
            Step::Skills => self.draw_skills(fb, skills),
            Step::Done => self.draw_done(fb),
            Step::Finished => {}
        }
    }

    fn draw_welcome(&mut self, fb: &Surface) {
        // Apple opens on a single word and nothing else.
        let w = fb.width() as i32;
        let cy = fb.height() as i32 / 2 - 40;
        fb.draw_text_centered(w / 2, cy, "hello", &HERO_FACE, font::HERO_TRACK, theme::INK);
        self.primary(fb, cy + 90, "Continue");
    }

    fn draw_bridge(&mut self, fb: &Surface, online: bool) {
        let top = self.header(
            fb,
            "Connect the Bridge",
            "Connectors run on the host, never in the kernel.",
        );
        self.status_card(fb, top, online);
        self.footer(fb, top + ROW_H + 8);
    }

    fn draw_caps(&mut self, fb: &Surface) {
        let top = self.header(
            fb,
            "Capabilities",
            "Every tool sits behind a grant. Turn on only what you need.",
        );
        let mut y = top;
        for i in 0..Cap::ALL.len() {
            self.cap_row(fb, y, i);
            y += ROW_H + 8;
        }
        self.footer(fb, y);
    }

    fn draw_skills(&mut self, fb: &Surface, skills: &SkillPeek) {
        let w = fb.width() as i32;
        let top = self.header(
            fb,
            "Default Skills",
            if skills.from_bridge() {
                if skills.count() == 0 {
                    "Bridge listed no skills. Tap Continue when ready."
                } else {
                    "Live from the host bridge. Tap Continue when ready."
                }
            } else {
                "Markdown playbooks the agent can load. Editable later."
            },
        );
        let mut y = top;
        for i in 0..skills.count().min(4) {
            ui::draw_titled_row(
                fb,
                content_rect(w, y, ROW_H),
                skills.name_at(i),
                skills.subtitle_at(i),
            );
            y += ROW_H + 8;
        }
        self.footer(fb, y);
    }

    fn draw_done(&mut self, fb: &Surface) {
        let w = fb.width() as i32;
        let cy = fb.height() as i32 / 2 - 40;
        fb.draw_text_centered(
            w / 2,
            cy,
            "You're all set.",
            &TITLE_FACE,
            font::TITLE_TRACK,
            theme::INK,
        );
        fb.draw_text_centered(
            w / 2,
            cy + 40,
            "Capabilities granted. Skills loaded.",
            &BODY_FACE,
            0,
            theme::MUTED,
        );
        self.primary(fb, cy + 90, "Start");
        self.back_link(fb, cy + 90 + CTA_H + 30);
    }

    // --- shared chrome -----------------------------------------------------

    /// Title + subtitle. Returns the y where content should start.
    fn header(&self, fb: &Surface, title: &str, sub: &str) -> i32 {
        let w = fb.width() as i32;
        let y = 132;
        fb.draw_text_centered(w / 2, y, title, &TITLE_FACE, font::TITLE_TRACK, theme::INK);
        fb.draw_text_centered(w / 2, y + 36, sub, &BODY_FACE, 0, theme::MUTED);
        y + 76
    }

    /// Capability toggle at index `i`: titled row + switch + hit zone.
    fn cap_row(&mut self, fb: &Surface, y: i32, i: usize) {
        let r = content_rect(fb.width() as i32, y, ROW_H);
        ui::draw_titled_row(fb, r, Cap::ALL[i].name(), Cap::ALL[i].blurb());
        ui::draw_switch_in_row(fb, r, self.caps.allows(Cap::ALL[i]));
        self.push_zone(r, Action::Row(i));
    }

    fn status_card(&self, fb: &Surface, y: i32, online: bool) {
        let (detail, tint) = if online {
            ("Connected on COM2", theme::ONLINE)
        } else {
            (crate::mcp::BRIDGE_OFFLINE_HINT, theme::OFFLINE)
        };
        let r = content_rect(fb.width() as i32, y, ROW_H + 8);
        ui::outlined_round_rect(fb, r, 10);
        let pad = 18;
        let d = 9;
        fb.fill_round_rect(r.x + pad, y + (ROW_H + 8 - d) / 2, d, d, d / 2, tint);
        fb.draw_text(r.x + pad + d + 12, y + 26, "Host bridge", &BRAND_FACE, 0, theme::INK);
        fb.draw_text(r.x + pad + d + 12, y + 46, detail, &SMALL_FACE, 0, theme::MUTED);
    }

    /// Primary pill, centred, registering a Continue zone.
    fn primary(&mut self, fb: &Surface, y: i32, label: &str) {
        let w = fb.width() as i32;
        let pad = 40;
        let bw = (BTN_FACE.width(label, 0) + pad * 2).max(180);
        let x = w / 2 - bw / 2;
        fb.fill_round_rect(x, y, bw, CTA_H, CTA_H / 2, theme::ACCENT);
        let base = y + (CTA_H - BTN_FACE.px) / 2 + BTN_FACE.ascent - 2;
        fb.draw_text_centered(w / 2, base, label, &BTN_FACE, 0, theme::SURFACE);
        self.push_zone(ui::Rect::new(x, y, bw, CTA_H), Action::Continue);
    }

    fn back_link(&mut self, fb: &Surface, y: i32) {
        let w = fb.width() as i32;
        let label = "Go Back";
        let tw = BTN_FACE.width(label, 0);
        let x = w / 2 - tw / 2;
        fb.draw_text(x, y + BTN_FACE.ascent, label, &BTN_FACE, 0, theme::ACCENT);
        // Generous target: the text alone is a 15px-tall sliver.
        self.push_zone(
            ui::Rect::new(x - 12, y - 8, tw + 24, BTN_FACE.px + 20),
            Action::Back,
        );
    }

    /// `content_bottom` = y just below the last row/card: on short
    /// framebuffers the pill moves down rather than overlapping the rows
    /// (zones are hit first-match, so an overlap misroutes clicks).
    fn footer(&mut self, fb: &Surface, content_bottom: i32) {
        let y = (fb.height() as i32 - 150).max(content_bottom + 24);
        self.primary(fb, y, "Continue");
        self.back_link(fb, y + CTA_H + 26);
    }
}

const CONTENT_W: i32 = 460;
const ROW_H: i32 = 58;
const CTA_H: i32 = 44;

fn content_rect(w: i32, y: i32, h: i32) -> ui::Rect {
    let cw = CONTENT_W.min(w - 80);
    ui::Rect::new((w - cw) / 2, y, cw, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journey_runs_forward_to_finished() {
        let mut s = Setup::new();
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
        let mut s = Setup::new();
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
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Welcome, "must not walk off the front");
    }

    #[test]
    fn finished_is_terminal() {
        let mut s = Setup::new();
        s.step = Step::Finished;
        s.apply(Action::Continue);
        s.apply(Action::Back);
        assert_eq!(s.step, Step::Finished);
    }

    #[test]
    fn rows_do_nothing_on_steps_without_rows() {
        let mut s = Setup::new();
        s.step = Step::Welcome;
        assert!(!s.apply(Action::Row(0)));
    }

    #[test]
    fn click_on_a_zone_applies_its_action() {
        let mut s = Setup::new();
        s.push_zone(ui::Rect::new(0, 0, 100, 100), Action::Continue);
        assert!(s.click(10, 10), "press should act");
        assert_eq!(s.step, Step::Bridge);
    }

    #[test]
    fn clicks_outside_any_zone_are_ignored() {
        let mut s = Setup::new();
        s.push_zone(ui::Rect::new(0, 0, 50, 50), Action::Continue);
        assert!(!s.click(400, 400));
        assert_eq!(s.step, Step::Welcome);
    }

    #[test]
    fn zone_table_cannot_overflow() {
        let mut s = Setup::new();
        for _ in 0..MAX_ZONES * 3 {
            s.push_zone(ui::Rect::new(0, 0, 10, 10), Action::Continue);
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
            all.push(cap.blurb());
        }
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII copy renders as '?': {s:?}"
            );
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
    fn there_is_headroom_for_one_more_capability() {
        // Overlapping rows and the Continue pill would misroute clicks — the
        // exact failure a previous review caught on a short framebuffer.
        // Asserts ALL+1 so the next capability addition fails here, not on
        // screen; that also covers today's ALL rows and the skills step
        // (capped at 4 < Cap::ALL.len()).
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
            // Round-trip: second click restores the prior grant.
            assert!(s.apply(Action::Row(i)), "row {i} second click did nothing");
            assert_eq!(s.caps.allows(cap), before, "row {i} did not toggle back");
        }
    }

    #[test]
    fn capability_names_and_blurbs_fit_the_column() {
        for cap in Cap::ALL {
            let name = cap.name();
            let blurb = cap.blurb();
            assert!(BRAND_FACE.width(name, 0) < CONTENT_W - 76, "name hits the switch: {name}");
            assert!(SMALL_FACE.width(blurb, 0) < CONTENT_W - 76, "blurb hits the switch: {blurb}");
        }
    }
}
