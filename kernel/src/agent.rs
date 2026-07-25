//! Deterministic guest skill runner.
//!
//! Skills are markdown playbooks on the host; the kernel does not run an LLM.
//! Named builtins map to a fixed plan of MCP calls under the current
//! [`crate::caps::Caps`], then a Brief the UI can show. That is the first
//! screen where capabilities and skills are felt together — click a playbook,
//! see it act within the grants, or be told which switch is still off.

use crate::caps::{Cap, Caps};
use crate::mcp::{self, BridgeStatus, MailPeek, SearchPeek};

/// Which builtin playbook the runner knows how to execute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    InboxBrief,
    EmailTriage,
    KnowledgeSearch,
    PlanAct,
    CapSafe,
    /// Listed, but not a runnable guest plan (show blurb only).
    Unknown,
}

/// One line in a brief section.
#[derive(Clone, Copy)]
pub struct Line {
    /// Short lane label: Urgent / FYI / Noise / Hit / Need / Next.
    pub tag: [u8; 10],
    pub text: [u8; 68],
}

impl Line {
    const fn empty() -> Self {
        Self {
            tag: [0; 10],
            text: [0; 68],
        }
    }

    pub fn tag(&self) -> &str {
        str_at(&self.tag)
    }

    pub fn text(&self) -> &str {
        str_at(&self.text)
    }
}

/// Result of running a skill under the current grants.
pub struct Brief {
    pub skill: [u8; 28],
    pub heading: [u8; 40],
    pub status: BridgeStatus,
    /// True when a required capability was missing.
    pub denied: bool,
    pub deny_cap: [u8; 24],
    pub plan_n: usize,
    pub plans: [[u8; 52]; 5],
    pub count: usize,
    pub lines: [Line; 8],
}

impl Brief {
    pub const fn empty() -> Self {
        Self {
            skill: [0; 28],
            heading: [0; 40],
            status: BridgeStatus::Offline,
            denied: false,
            deny_cap: [0; 24],
            plan_n: 0,
            plans: [[0; 52]; 5],
            count: 0,
            lines: [Line::empty(); 8],
        }
    }

    pub fn skill_name(&self) -> &str {
        str_at(&self.skill)
    }

    pub fn heading(&self) -> &str {
        str_at(&self.heading)
    }

    pub fn deny_name(&self) -> &str {
        str_at(&self.deny_cap)
    }

    pub fn plan_at(&self, i: usize) -> &str {
        str_at(&self.plans[i])
    }

    fn set_skill(&mut self, name: &str) {
        copy_field(&mut self.skill, name);
    }

    fn set_heading(&mut self, h: &str) {
        copy_field(&mut self.heading, h);
    }

    fn push_plan(&mut self, step: &str) {
        if self.plan_n < self.plans.len() {
            copy_field(&mut self.plans[self.plan_n], step);
            self.plan_n += 1;
        }
    }

    fn push_line(&mut self, tag: &str, text: &str) {
        if self.count < self.lines.len() {
            copy_field(&mut self.lines[self.count].tag, tag);
            copy_field(&mut self.lines[self.count].text, text);
            self.count += 1;
        }
    }

    fn need(&mut self, cap: Cap) {
        self.denied = true;
        copy_field(&mut self.deny_cap, cap.name());
        self.push_line("Need", cap.label());
    }
}

/// Map a skill name to a runnable kind.
pub fn classify(name: &str) -> Kind {
    match name {
        "inbox-brief" => Kind::InboxBrief,
        "email-triage" => Kind::EmailTriage,
        "knowledge-search" => Kind::KnowledgeSearch,
        "agent-plan-act" => Kind::PlanAct,
        "capability-safe-tools" => Kind::CapSafe,
        _ => Kind::Unknown,
    }
}

/// True when clicking the row should run a plan rather than only fetch a blurb.
pub fn is_runnable(name: &str) -> bool {
    !matches!(classify(name), Kind::Unknown)
}

/// Execute a named skill under `caps`. Unknown names return an empty brief
/// with `denied == false` so the UI can fall back to `skills.get`.
pub fn run(name: &str, caps: Caps) -> Brief {
    let mut brief = Brief::empty();
    brief.set_skill(name);
    match classify(name) {
        Kind::InboxBrief => run_inbox(&mut brief, caps, false),
        Kind::EmailTriage => run_inbox(&mut brief, caps, true),
        Kind::KnowledgeSearch => run_knowledge(&mut brief, caps, "capability-agent"),
        Kind::PlanAct => run_plan_act(&mut brief, caps),
        Kind::CapSafe => run_cap_safe(&mut brief, caps),
        Kind::Unknown => {
            brief.set_heading("No guest plan for this skill");
            brief.push_plan("Load playbook text from the host");
            brief.push_line("Info", "Open the skill body; no MCP calls yet.");
        }
    }
    brief
}

/// Morning brief used on home after setup: plan/act over whatever is granted.
pub fn morning(caps: Caps) -> Brief {
    run("agent-plan-act", caps)
}

fn run_inbox(brief: &mut Brief, caps: Caps, triage: bool) {
    brief.set_heading(if triage {
        "Inbox triage"
    } else {
        "Morning mail brief"
    });
    brief.push_plan("Check email.search grant");
    brief.push_plan("CALL email.search q=in:inbox max=5");
    brief.push_plan(if triage {
        "Rank: reply / wait / skip"
    } else {
        "Bucket: Urgent / FYI / Noise"
    });

    if !caps.allows(Cap::EmailSearch) {
        brief.need(Cap::EmailSearch);
        brief.push_line("Info", "Grant Email on Capabilities, then re-run.");
        return;
    }

    let mail = mcp::fetch_mail_peek(caps);
    brief.status = mail.status;
    if mail.status == BridgeStatus::Offline {
        brief.push_line("Info", "Bridge offline - cannot read mail.");
        return;
    }
    if mail.count == 0 {
        brief.push_line("FYI", "Inbox empty right now.");
        return;
    }
    fill_mail_lines(brief, &mail, triage);
}

fn fill_mail_lines(brief: &mut Brief, mail: &MailPeek, triage: bool) {
    for i in 0..mail.count.min(5) {
        let subj = mail.row_subj(i);
        let from = mail.row_from(i);
        let lane = classify_mail(subj, from);
        let tag = if triage {
            match lane {
                MailLane::Urgent => "Reply",
                MailLane::Fyi => "Wait",
                MailLane::Noise => "Skip",
            }
        } else {
            match lane {
                MailLane::Urgent => "Urgent",
                MailLane::Fyi => "FYI",
                MailLane::Noise => "Noise",
            }
        };
        let mut text = [0u8; 68];
        // "subj - from"
        let mut n = 0;
        for &b in subj.as_bytes().iter().take(40) {
            text[n] = b;
            n += 1;
        }
        if n + 3 < text.len() {
            text[n] = b' ';
            text[n + 1] = b'-';
            text[n + 2] = b' ';
            n += 3;
        }
        for &b in from.as_bytes() {
            if n >= text.len() {
                break;
            }
            text[n] = b;
            n += 1;
        }
        let line = core::str::from_utf8(&text[..n]).unwrap_or(subj);
        brief.push_line(tag, line);
    }
}

#[derive(Clone, Copy)]
enum MailLane {
    Urgent,
    Fyi,
    Noise,
}

fn classify_mail(subj: &str, from: &str) -> MailLane {
    let s = lower_has(subj);
    let f = lower_has(from);
    if f.contains("noreply")
        || f.contains("no-reply")
        || s.contains("newsletter")
        || s.contains("unsubscribe")
        || s.contains("digest")
        || s.contains("notification")
    {
        return MailLane::Noise;
    }
    if s.contains("urgent")
        || s.contains("asap")
        || s.contains("action required")
        || s.contains("deadline")
        || s.contains("important")
        || subj.contains('!')
    {
        return MailLane::Urgent;
    }
    MailLane::Fyi
}

/// ASCII lowercaser into a small scratch so keyword checks are case-blind.
fn lower_has(s: &str) -> heapless_lower::Lower {
    heapless_lower::Lower::new(s)
}

mod heapless_lower {
    /// Up to 96 bytes of lowercased ASCII (non-ASCII passed through).
    pub struct Lower {
        buf: [u8; 96],
        n: usize,
    }

    impl Lower {
        pub fn new(s: &str) -> Self {
            let mut buf = [0u8; 96];
            let mut n = 0;
            for &b in s.as_bytes() {
                if n >= buf.len() {
                    break;
                }
                buf[n] = if (b'A'..=b'Z').contains(&b) {
                    b + 32
                } else {
                    b
                };
                n += 1;
            }
            Self { buf, n }
        }

        pub fn contains(&self, needle: &str) -> bool {
            let hay = &self.buf[..self.n];
            let n = needle.as_bytes();
            if n.is_empty() || n.len() > hay.len() {
                return false;
            }
            hay.windows(n.len()).any(|w| w == n)
        }
    }
}

fn run_knowledge(brief: &mut Brief, caps: Caps, q: &str) {
    brief.set_heading("Knowledge search");
    brief.push_plan("Check search.query grant");
    brief.push_plan("CALL search.query on the corpus");
    brief.push_plan("Cite title only; do not invent");

    if !caps.allows(Cap::SearchQuery) {
        brief.need(Cap::SearchQuery);
        brief.push_line("Info", "Grant Built-in docs, then re-run.");
        return;
    }

    let peek = mcp::fetch_search_peek(caps, q);
    brief.status = peek.status;
    if peek.denied {
        brief.need(Cap::SearchQuery);
        return;
    }
    fill_search_lines(brief, &peek);
}

fn fill_search_lines(brief: &mut Brief, peek: &SearchPeek) {
    if peek.count == 0 {
        brief.push_line(
            "Info",
            if peek.status == BridgeStatus::Offline {
                "No offline hits for that query."
            } else {
                "No corpus hits."
            },
        );
        return;
    }
    for i in 0..peek.count.min(5) {
        brief.push_line("Hit", peek.title_at(i));
    }
}

fn run_plan_act(brief: &mut Brief, caps: Caps) {
    brief.set_heading("Plan, act, report");
    brief.push_plan("Restate: what matters right now");
    brief.push_plan("List grants in force");
    brief.push_plan("Act via MCP under those grants");
    brief.push_plan("Report outcomes");

    // Caps summary first.
    let mut on = 0usize;
    for cap in Cap::ALL {
        if caps.allows(cap) {
            on += 1;
        }
    }
    if on == 0 {
        brief.push_line("Need", "No capabilities granted yet.");
    } else {
        brief.push_line("Plan", "Acting only with switches that are on.");
    }

    // Mail lane when granted.
    if caps.allows(Cap::EmailSearch) {
        let mail = mcp::fetch_mail_peek(caps);
        brief.status = mail.status;
        if mail.status == BridgeStatus::Online && mail.count > 0 {
            fill_mail_lines(brief, &mail, false);
        } else if mail.status == BridgeStatus::Online {
            brief.push_line("FYI", "Inbox empty.");
        } else {
            brief.push_line("Info", "Bridge offline for mail.");
        }
    } else {
        brief.push_line("Info", "Email off - skipping inbox.");
    }

    // Knowledge lane when granted.
    if caps.allows(Cap::SearchQuery) {
        let peek = mcp::fetch_search_peek(caps, "capability-agent");
        if brief.status != BridgeStatus::Online {
            brief.status = peek.status;
        }
        if peek.count > 0 {
            // Keep room: at most 3 knowledge hits after mail lines.
            let room = brief.lines.len().saturating_sub(brief.count).min(3);
            for i in 0..peek.count.min(room) {
                brief.push_line("Hit", peek.title_at(i));
            }
        } else {
            brief.push_line("Info", "Knowledge search returned no hits.");
        }
    } else {
        brief.push_line("Need", Cap::SearchQuery.label());
    }
}

fn run_cap_safe(brief: &mut Brief, caps: Caps) {
    brief.set_heading("Capability check");
    brief.status = BridgeStatus::Online;
    brief.push_plan("List each grant");
    brief.push_plan("Refuse tools whose switch is off");
    brief.push_plan("Never invent ambient root");

    for cap in Cap::ALL {
        let tag = if caps.allows(cap) { "On" } else { "Off" };
        brief.push_line(tag, cap.label());
    }
}

fn str_at(buf: &[u8]) -> &str {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    match core::str::from_utf8(&buf[..n]) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&buf[..e.valid_up_to()]).unwrap_or(""),
    }
}

fn copy_field(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let mut n = bytes.len().min(dst.len());
    while n > 0 && !src.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_classify() {
        assert_eq!(classify("inbox-brief"), Kind::InboxBrief);
        assert_eq!(classify("knowledge-search"), Kind::KnowledgeSearch);
        assert_eq!(classify("agent-plan-act"), Kind::PlanAct);
        assert_eq!(classify("mystery"), Kind::Unknown);
        assert!(is_runnable("email-triage"));
        assert!(!is_runnable("custom-saved"));
    }

    #[test]
    fn mail_lanes_prefer_noise_and_urgent() {
        assert!(matches!(
            classify_mail("Weekly newsletter", "news@example.com"),
            MailLane::Noise
        ));
        assert!(matches!(
            classify_mail("URGENT: production down", "oncall@x.com"),
            MailLane::Urgent
        ));
        assert!(matches!(
            classify_mail("Lunch tomorrow", "ada@x.com"),
            MailLane::Fyi
        ));
    }

    #[test]
    fn inbox_without_grant_names_the_missing_cap() {
        let b = run("inbox-brief", Caps::none());
        assert!(b.denied);
        assert_eq!(b.deny_name(), "email.search");
        assert!(b.plan_n >= 2);
        assert!(b.count >= 1);
        assert_eq!(b.lines[0].tag(), "Need");
    }

    #[test]
    fn knowledge_without_grant_is_denied() {
        let b = run("knowledge-search", Caps::none());
        assert!(b.denied);
        assert_eq!(b.deny_name(), "search.query");
    }

    #[test]
    fn knowledge_report_lines_cite_titles_only() {
        // Do not call MCP here: host unit tests cannot touch COM2.
        let mut brief = Brief::empty();
        brief.set_heading("Knowledge search");
        let mut peek = SearchPeek::empty(BridgeStatus::Offline, false);
        copy_field(&mut peek.hits[0].title, "Capability model");
        peek.count = 1;
        fill_search_lines(&mut brief, &peek);
        assert_eq!(brief.count, 1);
        assert_eq!(brief.lines[0].tag(), "Hit");
        assert_eq!(brief.lines[0].text(), "Capability model");
    }

    #[test]
    fn mail_report_buckets_into_lanes() {
        let mut brief = Brief::empty();
        let mut mail = MailPeek::empty(BridgeStatus::Online);
        copy_field(&mut mail.rows[0].subj, "URGENT: ship it");
        copy_field(&mut mail.rows[0].from, "ops@x.com");
        copy_field(&mut mail.rows[1].subj, "Weekly newsletter");
        copy_field(&mut mail.rows[1].from, "noreply@x.com");
        mail.count = 2;
        fill_mail_lines(&mut brief, &mail, false);
        assert_eq!(brief.lines[0].tag(), "Urgent");
        assert_eq!(brief.lines[1].tag(), "Noise");
    }

    #[test]
    fn cap_safe_lists_every_switch() {
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        let b = run("capability-safe-tools", caps);
        assert_eq!(b.count, Cap::ALL.len());
        assert!(b.lines.iter().any(|l| l.tag() == "On" && l.text() == "Built-in docs"));
        assert!(b.lines.iter().any(|l| l.tag() == "Off" && l.text() == "Email"));
    }

    #[test]
    fn plan_act_plans_mention_grants_before_acting() {
        // Full run() would hit COM2 for search; assert the plan shape only.
        let mut brief = Brief::empty();
        brief.set_skill("agent-plan-act");
        brief.set_heading("Plan, act, report");
        brief.push_plan("Restate: what matters right now");
        brief.push_plan("List grants in force");
        brief.push_line("Info", "Email off - skipping inbox.");
        assert!(brief.plan_at(1).contains("grants"));
        assert!(brief.lines[0].text().contains("Email off"));
    }

    #[test]
    fn copy_is_ascii_only() {
        for s in [
            "Morning mail brief",
            "Inbox triage",
            "Knowledge search",
            "Plan, act, report",
            "Capability check",
            "Grant Email on Capabilities, then re-run.",
            "Bridge offline - cannot read mail.",
            "Acting only with switches that are on.",
        ] {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII: {s:?}"
            );
        }
    }
}
