//! The Skills, Capabilities, and Brief screens.
//!
//! Both cards on the home page used to just log to COM1. They now open real
//! views that share the search screen's chrome: a Back affordance, a title,
//! and a column of rows.
//!
//! Capabilities is not merely a report — the switches are live, so grants
//! chosen during setup can be changed afterwards without reinstalling.
//!
//! Brief is where a skill actually runs: plan lines, then results (or the
//! name of the switch still standing in the way).

use crate::agent::Brief;
use crate::caps::{Cap, Caps};
use crate::fb::Surface;
use crate::font::{self, BRAND_FACE, BTN_FACE, SMALL_FACE, TITLE_FACE};
use crate::keyboard::TextField;
use crate::level::Level;
use crate::searchui::back_rect;
use crate::setup::{N_CAPS, cap_rows};
#[cfg(test)]
use crate::skills::BUILTIN;
use crate::skills::{SkillPeek, Workflow};
use crate::ui::{Rect, hover_rail_rect, paint_hover_rail, theme};

/// Which full-screen view is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Home,
    Search,
    Skills,
    Caps,
    /// Reading a document opened from a search result.
    Reader,
    /// A skill just ran; show its plan and outcomes.
    Brief,
    /// Connection status for every source, opened from the nav dot.
    Status,
    /// A human-in-the-loop skill checklist.
    Playbook,
    /// Portal config, reached only by the Ctrl+Shift+P chord on Home.
    ///
    /// Nothing in the UI points here. It is for whoever set the machine up,
    /// not for the person it was handed to.
    PortalConfig,
}

const NAV_H: i32 = 56;
const PAD_X: i32 = 28;
const CONTENT_MAX: i32 = 720;
const ROW_H: i32 = 62;
const ROW_GAP: i32 = 8;
const TOP: i32 = 150;

fn column(w: i32) -> (i32, i32) {
    let cw = (w - PAD_X * 2).min(CONTENT_MAX);
    ((w - cw) / 2, cw)
}

/// Bounding box of row `i`, for both drawing and hit-testing.
pub fn row_rect(w: i32, i: usize) -> (i32, i32, i32, i32) {
    let (x, cw) = column(w);
    (x, TOP + i as i32 * (ROW_H + ROW_GAP), cw, ROW_H)
}

/// Dirty region used for a list row's hover feedback.
pub fn row_hover_rect(w: i32, i: usize) -> Rect {
    let (x, y, rw, rh) = row_rect(w, i);
    hover_rail_rect(Rect { x, y, w: rw, h: rh })
}

/// Repaint only one row's calm 2px intent rail.
///
/// Capabilities, Skills, and the hidden portal picker share this geometry, so
/// they get one interaction language without changing their full draw paths.
pub fn paint_row_hover(fb: &Surface, w: i32, i: usize, on: bool) {
    let (x, y, rw, rh) = row_rect(w, i);
    paint_hover_rail(fb, Rect { x, y, w: rw, h: rh }, on);
}

/// Which capability row contains this point, if any.
pub fn caps_hit(w: i32, x: i32, y: i32) -> Option<usize> {
    (0..N_CAPS).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

fn chrome(fb: &Surface, w: i32, title: &str, heading: &str) {
    fb.fill(theme::BG);
    let (bx, by, _, _) = back_rect(w);
    fb.draw_text(
        bx,
        by + BTN_FACE.baseline(),
        "Back",
        &BTN_FACE,
        0,
        theme::ACCENT,
    );
    fb.draw_text_centered(
        w / 2,
        (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline(),
        title,
        &BRAND_FACE,
        0,
        theme::INK,
    );
    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);
    let track = font::tracking_pct(TITLE_FACE.px, -20);
    fb.draw_text_centered(w / 2, 112, heading, &TITLE_FACE, track, theme::INK);
}

/// A bordered row with a title and a subtitle.
fn row(
    fb: &Surface,
    w: i32,
    i: usize,
    title: &str,
    sub: &str,
    accent: bool,
) -> (i32, i32, i32, i32) {
    let (x, y, cw, h) = row_rect(w, i);
    let border = if accent {
        theme::ACCENT
    } else {
        theme::CARD_BORDER
    };
    fb.draw_round_rect_outline(x, y, cw, h, 10, 1, border);
    fb.fill_round_rect(x + 1, y + 1, cw - 2, h - 2, 9, theme::BG);
    fb.draw_text(x + 18, y + 26, title, &BRAND_FACE, 0, theme::INK);
    fb.draw_text(x + 18, y + 46, sub, &SMALL_FACE, 0, theme::MUTED);
    (x, y, cw, h)
}

/// Skills the agent can load — names from bridge `skills.list`, else builtins.
///
/// When `Save skills` is granted, a footer CTA writes `guest-starter` via
/// `skills.save skills=1` so the cap is felt on this screen, not only in Brief.
pub fn draw_skills(fb: &Surface, peek: &SkillPeek, caps: Caps) {
    let w = fb.width() as i32;
    let heading = if caps.allows(Cap::SkillsSave) {
        "Run a playbook, or save a starter"
    } else {
        "Tap a playbook to run it"
    };
    chrome(fb, w, "Skills", heading);

    let n = peek.count.min(7);
    for i in 0..n {
        let name = peek.name_at(i);
        let desc = peek.desc_at(i);
        let sub = if !desc.is_empty() {
            desc
        } else if peek.is_saved_at(i) {
            "Saved - CALL granted tools"
        } else if crate::agent::is_runnable(name) {
            "Tap to run under current grants"
        } else if peek.from_bridge {
            crate::copy::skills_source()
        } else {
            "Shipped with the ISO"
        };
        let accent = crate::agent::is_runnable(name) || peek.is_saved_at(i);
        row(fb, w, i, name, sub, accent);
    }

    let mut note_row = n;
    if caps.allows(Cap::SkillsSave) {
        row(
            fb,
            w,
            n,
            "Save starter",
            "Write guest-starter to the host",
            true,
        );
        note_row = n + 1;
    }

    let (x, cw) = column(w);
    let note = if caps.allows(Cap::SkillsSave) {
        crate::copy::save_starter_hint()
    } else if peek.from_bridge {
        "Builtins run under grants. Saved CALL only tools you already turned on."
    } else if peek.count > 0 {
        "Tap a playbook: builtins run; saved CALL only granted tools."
    } else {
        "No skills loaded."
    };
    fb.draw_text(
        x,
        TOP + note_row as i32 * (ROW_H + ROW_GAP) + 26,
        note,
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
    let _ = cw;
}

/// Show the outcome of a skill run: plan, then tagged result lines.
pub fn draw_brief(fb: &Surface, brief: &Brief) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    let heading = if brief.heading().is_empty() {
        brief.skill_name()
    } else {
        brief.heading()
    };
    chrome(fb, w, "Brief", heading);

    let (x, cw) = column(w);
    let mut y = TOP - 24;

    if brief.denied {
        let mut msg = [0u8; 64];
        let prefix = b"Blocked: grant ";
        let mut n = 0;
        for &b in prefix {
            msg[n] = b;
            n += 1;
        }
        for &b in brief.deny_name().as_bytes() {
            if n < msg.len() {
                msg[n] = b;
                n += 1;
            }
        }
        let s = core::str::from_utf8(&msg[..n]).unwrap_or("Blocked by caps");
        fb.draw_text(x, y, s, &SMALL_FACE, 0, theme::ACCENT);
        y += 22;
    }

    if brief.plan_n > 0 {
        fb.draw_text(x, y, "Plan", &BRAND_FACE, 0, theme::INK);
        y += 24;
        for i in 0..brief.plan_n {
            // Numbered badge + step text on a single line.
            let badge_d = 18;
            let bx = x;
            let by_ = y - badge_d / 2;
            let badge_c = theme::TINT_BORDER;
            fb.fill_round_rect(bx, by_, badge_d, badge_d, badge_d / 2, badge_c);
            let digit: [u8; 1] = [b'1' + i as u8];
            let digit_str = core::str::from_utf8(&digit).unwrap_or("?");
            fb.draw_text_centered(
                bx + badge_d / 2,
                by_ + (badge_d - SMALL_FACE.px) / 2 + SMALL_FACE.baseline(),
                digit_str,
                &SMALL_FACE,
                0,
                theme::ACCENT,
            );
            fb.draw_text(x + badge_d + 8, y, brief.plan_at(i), &SMALL_FACE, 0, theme::MUTED);
            y += 20;
        }
        y += 8;
    }

    if brief.count > 0 {
        // Thin divider above the Report section when there was a Plan above it.
        if brief.plan_n > 0 {
            fb.fill_rect(x, y - 4, cw, 1, theme::RULE);
        }
        fb.draw_text(x, y, "Report", &BRAND_FACE, 0, theme::INK);
        y += 8;
        for i in 0..brief.count {
            let (rx, ry, rw, rh) = (
                x,
                y + 8 + i as i32 * (ROW_H - 10),
                cw,
                ROW_H - 14,
            );
            fb.draw_round_rect_outline(rx, ry, rw, rh, 10, 1, theme::CARD_BORDER);
            fb.fill_round_rect(rx + 1, ry + 1, rw - 2, rh - 2, 9, theme::BG);
            let tag = brief.lines[i].tag();
            let accent = matches!(tag, "Urgent" | "Reply" | "Need" | "Event" | "Doc");
            fb.draw_text(
                rx + 16,
                ry + 22,
                tag,
                &BRAND_FACE,
                0,
                if accent { theme::ACCENT } else { theme::MUTED },
            );
            fb.draw_text(
                rx + 16,
                ry + 42,
                brief.lines[i].text(),
                &SMALL_FACE,
                0,
                theme::INK,
            );
            let _ = rh;
        }
    }

    if brief.send_ready {
        let (sx, sy, sw, sh) = brief_send_rect(w, h);
        fb.draw_round_rect_outline(sx, sy, sw, sh, 10, 1, theme::ACCENT);
        fb.fill_round_rect(sx + 1, sy + 1, sw - 2, sh - 2, 9, theme::BG);
        fb.draw_text(sx + 18, sy + 26, "Confirm send", &BRAND_FACE, 0, theme::ACCENT);
        let mut sub = [0u8; 64];
        let mut n = 0;
        let prefix = b"To ";
        for &b in prefix {
            sub[n] = b;
            n += 1;
        }
        for &b in brief.draft_to().as_bytes().iter().take(28) {
            if n < sub.len() {
                sub[n] = b;
                n += 1;
            }
        }
        let s = core::str::from_utf8(&sub[..n]).unwrap_or("Draft ready");
        fb.draw_text(sx + 18, sy + 46, s, &SMALL_FACE, 0, theme::MUTED);
    }
}

/// Confirm send CTA on Brief — only when a draft was armed.
pub fn brief_send_rect(w: i32, h: i32) -> (i32, i32, i32, i32) {
    let (x, cw) = column(w);
    (x, h - 96, cw, ROW_H)
}

pub fn brief_send_hit(w: i32, h: i32, ready: bool, x: i32, y: i32) -> bool {
    if !ready {
        return false;
    }
    let (rx, ry, rw, rh) = brief_send_rect(w, h);
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

/// Y of the first Report row card — must match [`draw_brief`].
fn brief_report_top(brief: &Brief) -> i32 {
    let mut y = TOP - 24;
    if brief.denied {
        y += 22;
    }
    if brief.plan_n > 0 {
        y += 22 + brief.plan_n as i32 * 18 + 10;
    }
    if brief.count > 0 {
        y += 8; // "Report" heading
    }
    y
}

/// Hit an armed Event report row → index into [`Brief::event_url_at`].
pub fn brief_event_hit(w: i32, brief: &Brief, x: i32, y: i32) -> Option<usize> {
    brief_armed_hit(w, brief, x, y, brief.event_n, |b, i| b.event_line_at(i))
}

/// Hit an armed Doc report row → index into [`Brief::doc_url_at`].
pub fn brief_doc_hit(w: i32, brief: &Brief, x: i32, y: i32) -> Option<usize> {
    brief_armed_hit(w, brief, x, y, brief.doc_n, |b, i| b.doc_line_at(i))
}

fn brief_armed_hit(
    w: i32,
    brief: &Brief,
    x: i32,
    y: i32,
    n: usize,
    line_at: fn(&Brief, usize) -> Option<usize>,
) -> Option<usize> {
    if n == 0 || brief.count == 0 {
        return None;
    }
    let (col_x, cw) = column(w);
    let top = brief_report_top(brief);
    for i in 0..n {
        let Some(line_i) = line_at(brief, i) else {
            continue;
        };
        if line_i >= brief.count {
            continue;
        }
        let ry = top + 8 + line_i as i32 * (ROW_H - 10);
        let rh = ROW_H - 14;
        if x >= col_x && x < col_x + cw && y >= ry && y < ry + rh {
            return Some(i);
        }
    }
    None
}

/// Click target for advancing a workflow. Back uses the common chrome target.
pub fn playbook_next_rect(w: i32, _step: usize, total: usize) -> (i32, i32, i32, i32) {
    let (x, cw) = column(w);
    let y = 198 + total.min(5) as i32 * (ROW_H + ROW_GAP) + 18;
    (x, y, cw.min(220), 42)
}

/// Render a playbook as a small, inspectable state machine. Steps never run
/// bridge calls from this screen; execution stays an explicit later action.
pub fn draw_playbook(
    fb: &Surface,
    flow: Workflow,
    step: usize,
    goal: &TextField<{ crate::searchui::QUERY_MAX }>,
    caret: bool,
    grants: Caps,
) {
    let w = fb.width() as i32;
    chrome(fb, w, "Playbook", flow.title);
    let (x, cw) = column(w);
    let gy = 130;
    fb.draw_round_rect_outline(x, gy, cw, 44, 10, 1, theme::RULE);
    fb.fill_round_rect(x + 1, gy + 1, cw - 2, 42, 9, theme::BG);
    let base = gy + (44 - SMALL_FACE.px) / 2 + SMALL_FACE.baseline();
    if goal.is_empty() {
        fb.draw_text(
            x + 14,
            base,
            "What do you want to do?",
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
    } else {
        fb.draw_text(x + 14, base, goal.as_str(), &SMALL_FACE, 0, theme::INK);
    }
    if caret {
        let cx = x + 14 + SMALL_FACE.width(goal.as_str(), 0) + 2;
        fb.fill_rect(cx, gy + 10, 1, 24, theme::INK);
    }
    let active = step.min(flow.steps.len().saturating_sub(1));
    for (i, text) in flow.steps.iter().take(5).enumerate() {
        let y = 198 + i as i32 * (ROW_H + ROW_GAP);
        let done = i < active;
        let is_active = i == active;
        let border = if is_active {
            theme::ACCENT
        } else {
            theme::CARD_BORDER
        };
        fb.draw_round_rect_outline(x, y, cw, ROW_H, 10, 1, border);
        fb.fill_round_rect(x + 1, y + 1, cw - 2, ROW_H - 2, 9, theme::BG);

        // Step number badge: filled circle with number for done/active, hollow for later.
        let badge_d = 24;
        let bx = x + cw - 18 - badge_d;
        let by_ = y + (ROW_H - badge_d) / 2;
        let badge_color = if done {
            theme::ACCENT
        } else if is_active {
            theme::ACCENT
        } else {
            theme::RULE
        };
        fb.fill_round_rect(bx, by_, badge_d, badge_d, badge_d / 2, badge_color);
        // Number glyph inside the badge.
        let digit: [u8; 1] = [b'1' + i as u8];
        let digit_str = core::str::from_utf8(&digit).unwrap_or("?");
        let glyph_color = if done || is_active { theme::BG } else { theme::MUTED };
        fb.draw_text_centered(
            bx + badge_d / 2,
            by_ + (badge_d - SMALL_FACE.px) / 2 + SMALL_FACE.baseline(),
            digit_str,
            &SMALL_FACE,
            0,
            glyph_color,
        );

        let text_color = if done { theme::MUTED } else { theme::INK };
        fb.draw_text_clipped(x + 18, y + 26, text, &BRAND_FACE, 0, text_color, cw - badge_d - 54);
        let state_label = if done {
            "Done"
        } else if is_active {
            "In progress"
        } else {
            "Pending"
        };
        let label_color = if is_active { theme::ACCENT } else { theme::MUTED };
        fb.draw_text(x + 18, y + 46, state_label, &SMALL_FACE, 0, label_color);
    }
    let (x, y, bw, bh) = playbook_next_rect(w, step, flow.steps.len());
    let done = step + 1 >= flow.steps.len();
    fb.fill_round_rect(
        x,
        y,
        bw,
        bh,
        bh / 2,
        if done {
            theme::TINT_BORDER
        } else {
            theme::ACCENT
        },
    );
    let action = if done {
        "Run approved plan"
    } else {
        "Next step"
    };
    fb.draw_text_centered(
        x + bw / 2,
        y + (bh - BTN_FACE.px) / 2 + BTN_FACE.baseline(),
        action,
        &BTN_FACE,
        0,
        if done { theme::ACCENT } else { theme::BG },
    );
    if let Some(cap) = flow.required {
        let note = if grants.allows(cap) {
            "Permission granted. Run only after you review the plan."
        } else {
            "Permission needed before the approved action can run."
        };
        fb.draw_text(x, y + bh + 24, note, &SMALL_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(
            x,
            y + bh + 24,
            "The final button sends this goal to the scoped agent.",
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
    }
}

/// Which skill row contains this point, if any.
pub fn skills_hit(w: i32, count: usize, x: i32, y: i32) -> Option<usize> {
    (0..count.min(7)).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

/// True when the click landed on the Save starter CTA (row after the list).
pub fn skills_save_hit(w: i32, count: usize, can_save: bool, x: i32, y: i32) -> bool {
    if !can_save {
        return false;
    }
    let i = count.min(7);
    let (rx, ry, rw, rh) = row_rect(w, i);
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

/// Live capability switches. Clicking a row toggles the grant.
pub fn draw_caps(fb: &Surface, grants: Caps, level: Level) {
    let w = fb.width() as i32;
    chrome(fb, w, "Capabilities", level.caps_subtitle());

    for (i, (name, blurb)) in cap_rows(level).enumerate() {
        let on = Cap::ALL.get(i).map(|c| grants.allows(*c)).unwrap_or(false);
        let (x, y, cw, h) = row(fb, w, i, name, blurb, false);

        // Pill switch, filled when granted.
        let tw = 40;
        let th = 22;
        let tx = x + cw - 18 - tw;
        let ty = y + (h - th) / 2;
        fb.fill_round_rect(
            tx,
            ty,
            tw,
            th,
            th / 2,
            if on { theme::ACCENT } else { theme::RULE },
        );
        let knob = th - 6;
        let kx = if on { tx + tw - knob - 3 } else { tx + 3 };
        fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::BG);
    }

    let (x, _) = column(w);
    fb.draw_text(
        x,
        TOP + N_CAPS as i32 * (ROW_H + ROW_GAP) + 26,
        level.caps_footer(),
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
}

/// Toggle capability `i`, returning the new grant set.
pub fn toggle(grants: Caps, i: usize) -> Caps {
    let mut g = grants;
    if let Some(c) = Cap::ALL.get(i) {
        g.set(*c, !g.allows(*c));
    }
    g
}

// --- portal config: the hidden screen --------------------------------------

use crate::mcp::{ConfigStatus, PortalFamily, PortalSetStatus, UnlockStatus, PASS_MAX};

/// Height of the password field, matching the playbook goal box.
const PASS_FIELD_H: i32 = 44;
/// Diameter of one masking dot, and the step between them.
const DOT_D: i32 = 9;
const DOT_PITCH: i32 = 16;
/// Most dots the field draws. A longer secret still types — the row of dots
/// just stops growing, so it can never run off the end of the box.
pub const PASS_DOTS: usize = 24;

/// What the screen is currently telling the person.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfigNote {
    /// Nothing to report; the draw code substitutes a state-appropriate hint.
    None,
    BadPass,
    NotConfigured,
    TooMany,
    Offline,
    /// The secret could not be framed onto the wire, so it was never sent.
    Unsendable,
    Saved,
    /// The host's per-connection unlock expired under us.
    Relocked,
    Failed,
}

impl ConfigNote {
    pub fn text(self) -> &'static str {
        match self {
            ConfigNote::None => "",
            ConfigNote::BadPass => "That password did not match. Try again.",
            ConfigNote::NotConfigured => "No portal password is set on this Mac yet.",
            ConfigNote::TooMany => "Too many attempts. Wait a moment, then retry.",
            ConfigNote::Offline => crate::copy::offline_remedy(),
            ConfigNote::Unsendable => "No spaces or line breaks in the password.",
            ConfigNote::Saved => "Saved.",
            ConfigNote::Relocked => "The session locked again. Enter the password.",
            ConfigNote::Failed => "The host refused that change.",
        }
    }

    /// Does this need the person to do something? Drives the note's colour.
    pub fn is_error(self) -> bool {
        !matches!(self, ConfigNote::None | ConfigNote::Saved)
    }
}

/// State behind the hidden portal screen.
///
/// The typed secret lives in `pass` and leaves this struct by exactly one
/// route: [`crate::mcp::config_unlock`], which writes it to COM2 and nowhere
/// else. The draw code is never given it — only [`Self::mask_len`].
pub struct PortalConfig {
    pub status: ConfigStatus,
    pass: TextField<PASS_MAX>,
    pub note: ConfigNote,
}

impl PortalConfig {
    pub const fn new() -> Self {
        Self {
            status: ConfigStatus::offline(),
            pass: TextField::new(),
            note: ConfigNote::None,
        }
    }

    /// Adopt a fresh `config.status`, dropping anything half-typed.
    ///
    /// Unlock is per-connection on the host, so this is the only honest source
    /// of `locked` — the screen re-reads rather than trusting what it believed
    /// a moment ago.
    pub fn refresh(&mut self, status: ConfigStatus) {
        self.status = status;
        self.pass.clear();
        self.note = if status.reachable {
            ConfigNote::None
        } else {
            // Say so up front rather than waiting for a submit to hang.
            ConfigNote::Offline
        };
    }

    /// Feed a key to the password field. True when something changed.
    pub fn type_key(&mut self, key: crate::keyboard::Key) -> bool {
        self.pass.apply(key)
    }

    /// How many dots the field will draw.
    ///
    /// This is the *only* thing the renderer learns about the secret. There is
    /// no accessor that hands the text to drawing code, which is what keeps
    /// "never echo the password" a property of the type rather than a habit.
    pub fn mask_len(&self) -> usize {
        self.pass.len().min(PASS_DOTS)
    }

    pub fn pass_is_empty(&self) -> bool {
        self.pass.is_empty()
    }

    /// Hand the secret to the bridge and fold the reply back in.
    ///
    /// Borrowing the secret only for the duration of the call keeps it out of
    /// every caller's hands — `main.rs` never sees it, so it cannot log it.
    pub fn submit(&mut self) -> UnlockStatus {
        let reply = crate::mcp::config_unlock(self.pass.as_str());
        self.apply_unlock(reply);
        reply
    }

    /// Fold an unlock reply into the screen state.
    pub fn apply_unlock(&mut self, r: UnlockStatus) {
        // The typed secret is dropped either way: it has done its job, and a
        // failed attempt should not leave it sitting on screen to be retried
        // by whoever walks up next.
        self.pass.clear();
        self.note = match r {
            UnlockStatus::Ok => {
                self.status.locked = false;
                ConfigNote::None
            }
            UnlockStatus::BadPass => ConfigNote::BadPass,
            UnlockStatus::NotConfigured => ConfigNote::NotConfigured,
            UnlockStatus::TooMany => ConfigNote::TooMany,
            UnlockStatus::Offline => ConfigNote::Offline,
            UnlockStatus::Unsendable => ConfigNote::Unsendable,
        };
    }

    /// Fold a `config.portal` reply into the screen state.
    pub fn apply_portal(&mut self, r: PortalSetStatus) {
        self.note = match r {
            PortalSetStatus::Ok(f) => {
                self.status.family = f;
                ConfigNote::Saved
            }
            PortalSetStatus::Locked => {
                // The host dropped our unlock. Go back to the password rather
                // than leaving a picker up that can no longer save anything.
                self.status.locked = true;
                self.pass.clear();
                ConfigNote::Relocked
            }
            PortalSetStatus::UnknownFamily => ConfigNote::Failed,
            PortalSetStatus::Offline => ConfigNote::Offline,
            PortalSetStatus::Failed => ConfigNote::Failed,
        };
    }
}

impl Default for PortalConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Paint `n` masking dots from `x`, centred on `cy`. Returns the x after them.
///
/// Takes a count, never a string: there is no code path from the secret to a
/// glyph, because the glyph renderer is never called with it.
fn draw_mask(fb: &Surface, x: i32, cy: i32, n: usize) -> i32 {
    let mut dx = x;
    for _ in 0..n {
        fb.fill_round_rect(dx, cy - DOT_D / 2, DOT_D, DOT_D, DOT_D / 2, theme::INK);
        dx += DOT_PITCH;
    }
    dx
}

/// The hidden portal screen: password when locked, family picker when not.
pub fn draw_portal_config(fb: &Surface, cfg: &PortalConfig, caret: bool) {
    let w = fb.width() as i32;
    if cfg.status.locked {
        draw_portal_locked(fb, w, cfg, caret);
    } else {
        draw_portal_unlocked(fb, w, cfg);
    }
}

fn draw_portal_locked(fb: &Surface, w: i32, cfg: &PortalConfig, caret: bool) {
    // `chrome` draws the nav "Back" — the visible way home from a screen no
    // one was told about.
    chrome(fb, w, "Portal", "This screen is locked");

    let (x, cw) = column(w);
    let y = TOP;
    fb.draw_round_rect_outline(x, y, cw, PASS_FIELD_H, 10, 1, theme::RULE);
    fb.fill_round_rect(x + 1, y + 1, cw - 2, PASS_FIELD_H - 2, 9, theme::BG);

    let base = y + (PASS_FIELD_H - SMALL_FACE.px) / 2 + SMALL_FACE.baseline();
    if cfg.pass_is_empty() {
        fb.draw_text(x + 16, base, "Password", &SMALL_FACE, 0, theme::MUTED);
    }
    let after = draw_mask(fb, x + 16, y + PASS_FIELD_H / 2, cfg.mask_len());
    if caret {
        fb.fill_rect(after + 2, y + 12, 2, PASS_FIELD_H - 24, theme::INK);
    }

    let note = if cfg.note == ConfigNote::None {
        "Type the password, then press Enter."
    } else {
        cfg.note.text()
    };
    fb.draw_text(
        x,
        y + PASS_FIELD_H + 28,
        note,
        &SMALL_FACE,
        0,
        if cfg.note.is_error() { theme::ACCENT } else { theme::MUTED },
    );
}

fn draw_portal_unlocked(fb: &Surface, w: i32, cfg: &PortalConfig) {
    chrome(fb, w, "Portal", "Where this machine looks");

    for (i, family) in PortalFamily::ALL.into_iter().enumerate() {
        let active = family == cfg.status.family;
        let sub = if active { "Active" } else { "Tap to use" };
        let (x, y, cw, h) = row(fb, w, i, family.label(), sub, active);

        // Filled dot on the active row, hollow ring otherwise — the same
        // right-hand slot the capability switches occupy, so the two screens
        // read as one family.
        let d = 18;
        let mx = x + cw - 18 - d;
        let my = y + (h - d) / 2;
        fb.fill_round_rect(
            mx,
            my,
            d,
            d,
            d / 2,
            if active { theme::ACCENT } else { theme::RULE },
        );
        if !active {
            fb.fill_round_rect(mx + 3, my + 3, d - 6, d - 6, (d - 6) / 2, theme::BG);
        }
    }

    let (x, _) = column(w);
    let note = if cfg.note == ConfigNote::None {
        "Pick where this machine looks for portal data."
    } else {
        cfg.note.text()
    };
    fb.draw_text(
        x,
        TOP + PortalFamily::ALL.len() as i32 * (ROW_H + ROW_GAP) + 26,
        note,
        &SMALL_FACE,
        0,
        if cfg.note.is_error() { theme::ACCENT } else { theme::MUTED },
    );
}

/// Which portal family row contains this point, if any.
pub fn portal_family_hit(w: i32, x: i32, y: i32) -> Option<usize> {
    (0..PortalFamily::ALL.len()).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stack_without_overlapping() {
        for i in 1..N_CAPS {
            let (_, prev_y, _, prev_h) = row_rect(1024, i - 1);
            let (_, y, _, _) = row_rect(1024, i);
            assert!(y >= prev_y + prev_h, "row {i} overlaps its predecessor");
        }
    }

    #[test]
    fn every_capability_row_is_hittable_at_its_centre() {
        for i in 0..N_CAPS {
            let (x, y, w, h) = row_rect(1024, i);
            assert_eq!(caps_hit(1024, x + w / 2, y + h / 2), Some(i));
        }
    }

    #[test]
    fn clicks_between_and_outside_rows_hit_nothing() {
        let (_, y, _, h) = row_rect(1024, 0);
        // In the gap below the first row.
        assert_eq!(caps_hit(1024, 512, y + h + ROW_GAP / 2), None);
        // Left of the column.
        assert_eq!(caps_hit(1024, 2, y + h / 2), None);
        // Above the first row.
        assert_eq!(caps_hit(1024, 512, TOP - 5), None);
    }

    #[test]
    fn row_hover_rail_stays_inside_the_shared_row() {
        for i in 0..N_CAPS {
            let rail = row_hover_rect(1024, i);
            let (x, y, w, h) = row_rect(1024, i);
            assert!(x <= rail.x && y <= rail.y);
            assert!(rail.x + rail.w <= x + w);
            assert!(rail.y + rail.h <= y + h);
            assert_eq!(rail.h, 2);
        }
    }

    #[test]
    fn row_hover_paints_and_erases_only_two_scanlines() {
        const W: usize = 320;
        const H: usize = 260;
        let mut buf = vec![theme::BG; W * H];
        let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W, H) };
        let rail = row_hover_rect(W as i32, 0);

        paint_row_hover(&fb, W as i32, 0, true);
        assert_eq!(
            fb.dirty_rect(),
            Some((rail.x, rail.y, rail.x + rail.w, rail.y + rail.h))
        );
        assert_eq!(fb.get_pixel(rail.x, rail.y), theme::TINT_BORDER);

        fb.clear_dirty();
        paint_row_hover(&fb, W as i32, 0, false);
        assert_eq!(
            fb.dirty_rect(),
            Some((rail.x, rail.y, rail.x + rail.w, rail.y + rail.h))
        );
        assert_eq!(fb.get_pixel(rail.x, rail.y), theme::BG);
    }

    #[test]
    fn toggle_flips_only_the_named_capability() {
        let g = Caps::none();
        let after = toggle(g, 0);
        assert!(after.allows(Cap::ALL[0]));
        assert!(
            !after.allows(Cap::ALL[1]),
            "toggling one must not affect another"
        );
        assert!(
            !toggle(after, 0).allows(Cap::ALL[0]),
            "must toggle back off"
        );
    }

    #[test]
    fn toggle_out_of_range_is_a_noop() {
        let g = Caps::default_grants();
        // Out-of-range row must leave the grant set untouched.
        let mut a = [0u8; 96];
        let mut b = [0u8; 96];
        let na = toggle(g, 99).describe(&mut a);
        let nb = g.describe(&mut b);
        assert_eq!(a[..na], b[..nb]);
    }

    #[test]
    fn caps_screen_labels_match_the_capability_list() {
        // The switch for row i reflects Cap::ALL[i]; a mismatch would show the
        // wrong state against the wrong name.
        assert_eq!(N_CAPS, Cap::ALL.len());
    }

    #[test]
    fn all_rows_fit_a_768_screen() {
        // Seven skill rows + Save starter CTA + a short note must clear 768.
        let n = BUILTIN.len().min(7).max(N_CAPS);
        let (_, y, _, h) = row_rect(1024, n); // CTA under a full list
        assert!(y + h + 40 < 768, "rows run off the screen: {}", y + h);
    }

    #[test]
    fn skills_save_hit_only_when_writable() {
        let n = 3;
        let (x, y, w, h) = row_rect(1024, n);
        assert!(skills_save_hit(1024, n, true, x + w / 2, y + h / 2));
        assert!(!skills_save_hit(1024, n, false, x + w / 2, y + h / 2));
        assert!(!skills_save_hit(1024, n, true, x + w / 2, y - 4));
    }

    #[test]
    fn brief_send_hit_only_when_armed() {
        let (x, y, w, h) = brief_send_rect(1024, 768);
        assert!(brief_send_hit(1024, 768, true, x + w / 2, y + h / 2));
        assert!(!brief_send_hit(1024, 768, false, x + w / 2, y + h / 2));
        assert!(!brief_send_hit(1024, 768, true, x + w / 2, y - 4));
    }

    #[test]
    fn brief_event_rows_are_hittable() {
        let mut brief = Brief::empty();
        brief.push_report("Event", "Demo event - tomorrow");
        brief.arm_event("0123456789abcdef", 0);
        let top = brief_report_top(&brief);
        let (x, cw) = column(1024);
        let ry = top + 8;
        assert_eq!(
            brief_event_hit(1024, &brief, x + cw / 2, ry + 10),
            Some(0)
        );
        assert_eq!(brief_event_hit(1024, &Brief::empty(), x + cw / 2, ry + 10), None);
    }

    #[test]
    fn brief_doc_rows_are_hittable() {
        let mut brief = Brief::empty();
        brief.push_report("Goal", "work on paper");
        brief.push_report("Doc", "thesis-draft.md");
        brief.arm_doc("file://docs/thesis-draft.md", 1);
        let top = brief_report_top(&brief);
        let (x, cw) = column(1024);
        let ry = top + 8 + 1 * (ROW_H - 10);
        assert_eq!(
            brief_doc_hit(1024, &brief, x + cw / 2, ry + 10),
            Some(0)
        );
        assert_eq!(brief_doc_hit(1024, &Brief::empty(), x + cw / 2, ry + 10), None);
    }

    #[test]
    fn copy_is_ascii_only() {
        let mut all = vec![
            "Tap a playbook to run it",
            "Run a playbook, or save a starter",
            "Tap a playbook: builtins run; saved CALL only granted tools.",
            "Builtins run under grants. Saved CALL only tools you already turned on.",
            crate::copy::save_starter_hint(),
            "Playbook body unavailable.",
            "No skills loaded.",
            "Skills",
            "Capabilities",
            "Brief",
            "Back",
            crate::copy::skills_source(),
            "Shipped with the ISO",
            "Saved - CALL granted tools",
            "Save starter",
            "Write guest-starter to the host",
            "Tap to run under current grants",
            "Confirm send",
            "Plan",
            "Report",
        ];
        for level in Level::ALL {
            all.push(level.caps_subtitle());
            all.push(level.caps_footer());
        }
        for s in BUILTIN {
            all.push(s.name);
            all.push(s.blurb);
        }
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII renders as '?': {s:?}"
            );
        }
    }

    #[test]
    fn skill_text_fits_its_row() {
        let (_, _, cw, _) = row_rect(1024, 0);
        let peek = SkillPeek::from_builtin();
        for i in 0..peek.count.min(7) {
            assert!(
                BRAND_FACE.width(peek.name_at(i), 0) < cw - 36,
                "name overflows: {}",
                peek.name_at(i)
            );
            assert!(
                SMALL_FACE.width(peek.desc_at(i), 0) < cw - 36,
                "blurb overflows: {}",
                peek.desc_at(i)
            );
        }
    }

    #[test]
    fn skills_hit_finds_first_row() {
        let (x, y, _, _) = row_rect(1024, 0);
        assert_eq!(skills_hit(1024, 3, x + 4, y + 4), Some(0));
        assert_eq!(skills_hit(1024, 0, x + 4, y + 4), None);
    }

    #[test]
    fn playbook_next_target_fits_the_screen() {
        let (_, y, _, h) = playbook_next_rect(1024, 0, 5);
        assert!(y + h < 768);
    }

    #[test]
    fn capability_text_clears_the_switch() {
        let (_, _, cw, _) = row_rect(1024, 0);
        // Switch occupies the right 58px of the row.
        for level in Level::ALL {
            for (name, blurb) in cap_rows(level) {
                assert!(
                    BRAND_FACE.width(name, 0) < cw - 76,
                    "name hits the switch: {name}"
                );
                assert!(
                    SMALL_FACE.width(blurb, 0) < cw - 76,
                    "blurb hits the switch: {blurb}"
                );
            }
        }
    }
}

/// The hidden portal screen.
///
/// The load-bearing property is that the password is never painted. These
/// prove it against the real draw code rather than by reading it: two
/// different secrets of the same length must produce byte-identical frames.
#[cfg(test)]
mod portal_config_tests {
    use super::*;
    use crate::keyboard::Key;

    fn unlocked_at(family: PortalFamily) -> PortalConfig {
        let mut cfg = PortalConfig::new();
        cfg.refresh(ConfigStatus {
            reachable: true,
            family,
            locked: false,
            configured: true,
        });
        cfg
    }

    fn locked_with(secret: &str) -> PortalConfig {
        let mut cfg = PortalConfig::new();
        cfg.refresh(ConfigStatus {
            reachable: true,
            family: PortalFamily::None,
            locked: true,
            configured: true,
        });
        for b in secret.bytes() {
            cfg.type_key(Key::Char(b));
        }
        cfg
    }

    /// Render a locked screen carrying `secret` and return the raw pixels.
    fn frame(secret: &str) -> Vec<u32> {
        const W: usize = 1024;
        const H: usize = 768;
        let cfg = locked_with(secret);
        let mut buf = vec![0u32; W * H];
        {
            let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W, H) };
            // Caret fixed: this is about the secret, not the blink phase.
            draw_portal_config(&fb, &cfg, false);
        }
        buf
    }

    #[test]
    fn the_password_field_renders_masked() {
        // Same length, completely different characters. If a single glyph of
        // the secret reached the framebuffer these frames would differ.
        assert_eq!(frame("hunter7"), frame("SWORDF1"), "the secret is on screen");
        assert_eq!(frame("aaaaaaa"), frame("!@#$%^&"), "the secret is on screen");
    }

    #[test]
    fn the_mask_still_shows_that_something_was_typed() {
        // Guard the obvious way to pass the test above: drawing nothing.
        assert_ne!(frame("hunter7"), frame("hunter"), "the mask ignores length");
        assert_ne!(frame("a"), frame(""), "typing produced no visible dot");
    }

    #[test]
    fn the_mask_is_a_dot_per_character_and_stops_growing() {
        let cfg = locked_with("hunter2");
        assert_eq!(cfg.mask_len(), 7);
        // A very long secret must not run the dots off the end of the box.
        let long = locked_with(&"x".repeat(PASS_MAX));
        assert_eq!(long.mask_len(), PASS_DOTS.min(PASS_MAX));
        let (_, cw) = column(1024);
        assert!(
            16 + PASS_DOTS as i32 * DOT_PITCH < cw,
            "a full row of dots overflows the field"
        );
    }

    #[test]
    fn an_empty_field_draws_no_dots() {
        assert_eq!(PortalConfig::new().mask_len(), 0);
    }

    #[test]
    fn backspace_shortens_the_mask() {
        let mut cfg = locked_with("abc");
        assert_eq!(cfg.mask_len(), 3);
        assert!(cfg.type_key(Key::Backspace));
        assert_eq!(cfg.mask_len(), 2);
    }

    #[test]
    fn a_chord_is_not_typed_into_the_password() {
        // Otherwise the chord that opens this screen would seed the field.
        let mut cfg = locked_with("");
        assert!(!cfg.type_key(Key::Chord(b'P')));
        assert_eq!(cfg.mask_len(), 0);
    }

    #[test]
    fn the_picker_lists_all_three_choices_and_marks_the_active_one() {
        // Drive the real draw code, then check the rows it registered.
        const W: i32 = 1024;
        for active in PortalFamily::ALL {
            let cfg = unlocked_at(active);
            let mut buf = vec![0u32; (W * 768) as usize];
            {
                let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W as usize, 768) };
                draw_portal_config(&fb, &cfg, false);
            }
            // Three rows, each hittable at its centre and mapping to its own
            // index — the order the draw loop uses.
            for (i, _family) in PortalFamily::ALL.into_iter().enumerate() {
                let (x, y, w, h) = row_rect(W, i);
                assert_eq!(
                    portal_family_hit(W, x + w / 2, y + h / 2),
                    Some(i),
                    "row {i} is not clickable"
                );
            }
            assert_eq!(portal_family_hit(W, 512, TOP - 6), None, "hit above the list");
            // The active family is the one the status carries, and it is the
            // only row drawn with the accent marker.
            assert_eq!(cfg.status.family, active);
            let marked: Vec<_> = PortalFamily::ALL
                .into_iter()
                .filter(|f| *f == cfg.status.family)
                .collect();
            assert_eq!(marked.len(), 1, "exactly one row must read as active");
        }
    }

    #[test]
    fn all_three_rows_fit_a_768_screen() {
        let (_, y, _, h) = row_rect(1024, PortalFamily::ALL.len() - 1);
        assert!(y + h + 40 < 768, "the picker runs off the screen");
    }

    #[test]
    fn family_labels_clear_the_marker() {
        let (_, _, cw, _) = row_rect(1024, 0);
        for f in PortalFamily::ALL {
            assert!(
                BRAND_FACE.width(f.label(), 0) < cw - 76,
                "label hits the marker: {}",
                f.label()
            );
        }
    }

    #[test]
    fn a_bad_password_says_so_and_clears_the_field() {
        let mut cfg = locked_with("wrong");
        cfg.apply_unlock(UnlockStatus::BadPass);
        assert_eq!(cfg.note, ConfigNote::BadPass);
        assert!(cfg.note.is_error());
        assert!(cfg.status.locked, "a bad password must not unlock");
        assert_eq!(cfg.mask_len(), 0, "the failed secret stayed on screen");
    }

    #[test]
    fn the_other_unlock_failures_each_get_their_own_message() {
        for (reply, want) in [
            (UnlockStatus::NotConfigured, ConfigNote::NotConfigured),
            (UnlockStatus::TooMany, ConfigNote::TooMany),
            (UnlockStatus::Offline, ConfigNote::Offline),
            (UnlockStatus::Unsendable, ConfigNote::Unsendable),
        ] {
            let mut cfg = locked_with("secret");
            cfg.apply_unlock(reply);
            assert_eq!(cfg.note, want, "{reply:?} produced the wrong message");
            assert!(cfg.status.locked, "{reply:?} must leave the screen locked");
        }
    }

    #[test]
    fn a_good_password_opens_the_picker() {
        let mut cfg = locked_with("hunter2");
        cfg.apply_unlock(UnlockStatus::Ok);
        assert!(!cfg.status.locked);
        assert_eq!(cfg.mask_len(), 0, "the secret outlived the unlock");
        assert!(!cfg.note.is_error());
    }

    #[test]
    fn choosing_a_family_reflects_what_the_host_stored() {
        let mut cfg = unlocked_at(PortalFamily::None);
        cfg.apply_portal(PortalSetStatus::Ok(PortalFamily::Market));
        assert_eq!(cfg.status.family, PortalFamily::Market);
        assert_eq!(cfg.note, ConfigNote::Saved);
        assert!(!cfg.note.is_error());
    }

    #[test]
    fn a_lost_session_sends_us_back_to_the_password() {
        // Unlock is per-connection on the host; it can vanish under us.
        let mut cfg = unlocked_at(PortalFamily::Teddy);
        cfg.apply_portal(PortalSetStatus::Locked);
        assert!(cfg.status.locked, "a locked reply left the picker up");
        assert_eq!(cfg.note, ConfigNote::Relocked);
        assert_eq!(cfg.mask_len(), 0);
    }

    #[test]
    fn an_unreachable_bridge_says_so_instead_of_looking_ready() {
        let mut cfg = PortalConfig::new();
        cfg.refresh(ConfigStatus::offline());
        assert!(cfg.status.locked, "an offline bridge must read as locked");
        assert_eq!(cfg.note, ConfigNote::Offline);
        assert!(cfg.note.is_error());
        // And a failed set says it too, rather than hanging.
        let mut open = unlocked_at(PortalFamily::Teddy);
        open.apply_portal(PortalSetStatus::Offline);
        assert_eq!(open.note, ConfigNote::Offline);
    }

    #[test]
    fn drawing_both_states_does_not_panic() {
        for cfg in [locked_with("abc"), unlocked_at(PortalFamily::Teddy)] {
            for caret in [true, false] {
                let mut buf = vec![0u32; 1024 * 768];
                let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), 1024, 768) };
                draw_portal_config(&fb, &cfg, caret);
            }
        }
    }

    #[test]
    fn copy_is_ascii_only() {
        let mut all = vec![
            "Portal",
            "This screen is locked",
            "Where this machine looks",
            "Password",
            "Type the password, then press Enter.",
            "Pick where this machine looks for portal data.",
            "Active",
            "Tap to use",
        ];
        for n in [
            ConfigNote::None,
            ConfigNote::BadPass,
            ConfigNote::NotConfigured,
            ConfigNote::TooMany,
            ConfigNote::Offline,
            ConfigNote::Unsendable,
            ConfigNote::Saved,
            ConfigNote::Relocked,
            ConfigNote::Failed,
        ] {
            all.push(n.text());
        }
        for f in PortalFamily::ALL {
            all.push(f.label());
        }
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII renders as '?': {s:?}"
            );
        }
    }
}

/// Status of every source the OS can reach, opened from the nav dot.
///
/// The dot alone cannot say whether the portal is synced or the file index
/// exists; this is where the whole picture lives.
pub fn draw_status(
    fb: &Surface,
    mail: &crate::mcp::MailPeek,
    portal: &crate::mcp::PortalStatus,
    grants: Caps,
) {
    let w = fb.width() as i32;
    chrome(fb, w, "Status", "What this machine can reach");

    let (x, cw) = column(w);
    let mut y = TOP;

    let line = |fb: &Surface, y: i32, name: &str, state: &str, ok: bool| {
        fb.draw_round_rect_outline(x, y, cw, ROW_H, 10, 1, theme::CARD_BORDER);
        fb.fill_round_rect(x + 1, y + 1, cw - 2, ROW_H - 2, 9, theme::BG);
        let d = 9;
        fb.fill_round_rect(
            x + 18,
            y + (ROW_H - d) / 2,
            d,
            d,
            d / 2,
            if ok { theme::ONLINE } else { theme::OFFLINE },
        );
        fb.draw_text(x + 18 + d + 12, y + 26, name, &BRAND_FACE, 0, theme::INK);
        fb.draw_text(x + 18 + d + 12, y + 46, state, &SMALL_FACE, 0, theme::MUTED);
    };

    // Plain words: someone checking whether their machine works should not
    // need to know what COM2 is.
    let bridge_up = matches!(mail.status, crate::mcp::BridgeStatus::Online);
    line(
        fb,
        y,
        "This computer",
        if bridge_up {
            "Connected"
        } else {
            "Not connected"
        },
        bridge_up,
    );
    y += ROW_H + ROW_GAP;

    // Teddy has three distinct states and the dot cannot express them.
    let granted = grants.allows(Cap::PortalSync);
    let (teddy_state, teddy_ok) = if !granted {
        ("Off - turn on Online services", false)
    } else if portal.syncing {
        ("Downloading now", false)
    } else if !portal.cached {
        ("Not downloaded yet", false)
    } else if portal.docs == 0 {
        ("Downloaded, but empty", false)
    } else {
        ("Ready", true)
    };
    line(fb, y, "Teddy", teddy_state, teddy_ok);
    y += ROW_H + ROW_GAP;

    let files = grants.allows(Cap::WorkspaceIndex);
    line(
        fb,
        y,
        "Your files",
        if files { "Ready" } else { "Off" },
        files,
    );

    if granted && portal.cached {
        let mut buf = [0u8; 24];
        let n = fmt_usize(&mut buf, portal.docs);
        let txt = core::str::from_utf8(&buf[..n]).unwrap_or("");
        fb.draw_text(x, y + ROW_H + 34, txt, &SMALL_FACE, 0, theme::MUTED);
    }
}

/// "12448 documents cached" into a caller buffer.
fn fmt_usize(buf: &mut [u8; 24], mut v: usize) -> usize {
    let mut digits = [0u8; 10];
    let mut d = 0;
    if v == 0 {
        digits[0] = b'0';
        d = 1;
    }
    while v > 0 {
        digits[d] = b'0' + (v % 10) as u8;
        v /= 10;
        d += 1;
    }
    let mut n = 0;
    while d > 0 {
        d -= 1;
        buf[n] = digits[d];
        n += 1;
    }
    for &b in b" documents cached" {
        if n < buf.len() {
            buf[n] = b;
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use crate::mcp::{BridgeStatus, MailPeek, PortalStatus};

    fn mail_online() -> MailPeek {
        MailPeek {
            status: BridgeStatus::Online,
            ..MailPeek::empty(BridgeStatus::Online)
        }
    }

    fn mail_offline() -> MailPeek {
        MailPeek {
            status: BridgeStatus::Offline,
            ..MailPeek::empty(BridgeStatus::Offline)
        }
    }

    fn portal_ready() -> PortalStatus {
        PortalStatus {
            reachable: true,
            cached: true,
            docs: 42,
            syncing: false,
        }
    }

    #[test]
    fn drawing_status_screen_does_not_panic() {
        let mut buf = vec![0u32; 1024 * 768];
        let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), 1024, 768) };
        for online in [true, false] {
            let mail = if online { mail_online() } else { mail_offline() };
            let portal = portal_ready();
            draw_status(&fb, &mail, &portal, Caps::default_grants());
        }
    }

    #[test]
    fn status_rows_fit_a_768_screen() {
        // Three rows: bridge + teddy + files.
        let bottom_y = TOP + 3 * (ROW_H + ROW_GAP);
        assert!(bottom_y + 40 < 768, "status rows overflow 768px screen: {bottom_y}");
    }

    #[test]
    fn doc_count_note_only_appears_when_portal_is_cached() {
        const W: usize = 1024;
        const H: usize = 768;
        let mut buf_on = vec![0u32; W * H];
        let mut buf_off = vec![0u32; W * H];
        let grants = Caps::default_grants();

        {
            let fb = unsafe { Surface::in_memory(buf_on.as_mut_ptr(), W, H) };
            let mut grants_with = grants;
            grants_with.set(Cap::PortalSync, true);
            draw_status(&fb, &mail_online(), &portal_ready(), grants_with);
        }
        {
            let fb = unsafe { Surface::in_memory(buf_off.as_mut_ptr(), W, H) };
            draw_status(&fb, &mail_online(), &PortalStatus { reachable: true, cached: false, docs: 0, syncing: false }, grants);
        }
        // The frames must differ (one has the doc count note, the other doesn't).
        assert_ne!(buf_on, buf_off, "doc count note should change the frame");
    }

    #[test]
    fn fmt_usize_zero() {
        let mut buf = [0u8; 24];
        let n = fmt_usize(&mut buf, 0);
        let s = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.starts_with('0'), "zero should render as '0 ...' not empty");
    }

    #[test]
    fn fmt_usize_large() {
        let mut buf = [0u8; 24];
        let n = fmt_usize(&mut buf, 12_448);
        let s = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.starts_with("12448"), "wrong digits: {s}");
        assert!(s.contains("documents cached"));
    }

    #[test]
    fn fmt_usize_boundary_values() {
        for (v, want_prefix) in [
            (1, "1"),
            (9, "9"),
            (10, "10"),
            (99, "99"),
            (100, "100"),
            (999_999, "999999"),
        ] {
            let mut buf = [0u8; 24];
            let n = fmt_usize(&mut buf, v);
            let s = core::str::from_utf8(&buf[..n]).unwrap();
            assert!(s.starts_with(want_prefix), "v={v} rendered as {s}");
        }
    }

    #[test]
    fn screens_toggle_flips_grant_bit() {
        let init = Caps::none();
        assert!(!init.allows(Cap::EmailSearch));
        let toggled = toggle(init, 0);
        assert!(toggled.allows(Cap::EmailSearch));
        let toggled_back = toggle(toggled, 0);
        assert!(!toggled_back.allows(Cap::EmailSearch));
    }

    #[test]
    fn fmt_usize_pluralization() {
        let mut buf1 = [0u8; 24];
        let n1 = fmt_usize(&mut buf1, 1);
        let s1 = core::str::from_utf8(&buf1[..n1]).unwrap();
        assert_eq!(s1, "1 documents cached");

        let mut buf2 = [0u8; 24];
        let n2 = fmt_usize(&mut buf2, 5);
        let s2 = core::str::from_utf8(&buf2[..n2]).unwrap();
        assert_eq!(s2, "5 documents cached");
    }
}


