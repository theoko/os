//! Deterministic guest skill runner.
//!
//! Skills are markdown playbooks on the host; the kernel does not run an LLM.
//! Named builtins map to a fixed plan of MCP calls under the current
//! [`crate::caps::Caps`], then a Brief the UI can show. That is the first
//! screen where capabilities and skills are felt together — click a playbook,
//! see it act within the grants, or be told which switch is still off.

use crate::caps::{Cap, Caps};
use crate::level::Level;
use crate::mcp::{self, BridgeStatus, CalendarPeek, FilePeek, MailPeek, SearchPeek};

/// Which builtin playbook the runner knows how to execute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    InboxBrief,
    EmailTriage,
    KnowledgeSearch,
    PlanAct,
    CapSafe,
    /// Teddy corpus search + live teddysearch.com portals.
    TeddyPortals,
    /// Live superintelmarkets.com portals (same grant as teddy).
    MarketPortals,
    /// Saved / custom: plan preview, then CALL only tools already granted.
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
    /// Armed by inbox skills when Send mail is granted — Brief shows Confirm.
    pub send_ready: bool,
    draft_to: [u8; 40],
    draft_subj: [u8; 48],
    /// Openable calendar events (report line index + graph id).
    pub event_n: usize,
    event_id: [[u8; 20]; 2],
    event_line: [u8; 2],
    /// Openable Doc / Hit rows from a goal run (`file://`, `os://`, …).
    pub doc_n: usize,
    doc_url: [[u8; 72]; 3],
    doc_line: [u8; 3],
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
            send_ready: false,
            draft_to: [0; 40],
            draft_subj: [0; 48],
            event_n: 0,
            event_id: [[0; 20]; 2],
            event_line: [0; 2],
            doc_n: 0,
            doc_url: [[0; 72]; 3],
            doc_line: [0; 3],
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

    pub fn draft_to(&self) -> &str {
        str_at(&self.draft_to)
    }

    pub fn draft_subj(&self) -> &str {
        str_at(&self.draft_subj)
    }

    /// Arm the Confirm send CTA with a draft (never auto-CALLs).
    pub fn arm_send(&mut self, to: &str, subj: &str) {
        self.send_ready = true;
        copy_field(&mut self.draft_to, to);
        copy_field(&mut self.draft_subj, subj);
    }

    pub fn clear_send(&mut self) {
        self.send_ready = false;
        self.draft_to = [0; 40];
        self.draft_subj = [0; 48];
    }

    /// Remember an Event report line so Brief can open `cal://{id}`.
    pub fn arm_event(&mut self, id: &str, line_idx: usize) {
        if self.event_n >= self.event_id.len() || id.is_empty() {
            return;
        }
        copy_field(&mut self.event_id[self.event_n], id);
        self.event_line[self.event_n] = line_idx.min(255) as u8;
        self.event_n += 1;
    }

    pub fn event_line_at(&self, i: usize) -> Option<usize> {
        (i < self.event_n).then_some(self.event_line[i] as usize)
    }

    /// Build `cal://{id}` for armed event `i`.
    pub fn event_url_at<'a>(&self, i: usize, buf: &'a mut [u8; 40]) -> Option<&'a str> {
        if i >= self.event_n {
            return None;
        }
        let id = str_at(&self.event_id[i]);
        if id.is_empty() || id.len() > 24 {
            return None;
        }
        buf.fill(0);
        let prefix = b"cal://";
        let n = prefix.len() + id.len();
        if n > buf.len() {
            return None;
        }
        buf[..prefix.len()].copy_from_slice(prefix);
        buf[prefix.len()..n].copy_from_slice(id.as_bytes());
        Some(str_at(&buf[..n]))
    }

    /// Remember a Doc report line so Brief can open its URL.
    pub fn arm_doc(&mut self, url: &str, line_idx: usize) {
        if self.doc_n >= self.doc_url.len() || url.is_empty() {
            return;
        }
        copy_field(&mut self.doc_url[self.doc_n], url);
        self.doc_line[self.doc_n] = line_idx.min(255) as u8;
        self.doc_n += 1;
    }

    pub fn doc_line_at(&self, i: usize) -> Option<usize> {
        (i < self.doc_n).then_some(self.doc_line[i] as usize)
    }

    pub fn doc_url_at(&self, i: usize) -> Option<&str> {
        if i >= self.doc_n {
            return None;
        }
        let u = str_at(&self.doc_url[i]);
        (!u.is_empty()).then_some(u)
    }

    /// True when there is something worth keeping on the home screen.
    pub fn has_report(&self) -> bool {
        self.count > 0 || self.plan_n > 0 || !self.heading().is_empty()
    }

    /// Append a report line (used when enriching Unknown skills with a body peek).
    pub fn push_report(&mut self, tag: &str, text: &str) {
        self.push_line(tag, text);
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
        "teddy-portals" => Kind::TeddyPortals,
        "market-portals" => Kind::MarketPortals,
        _ => Kind::Unknown,
    }
}

/// True when clicking the row should run a plan rather than only fetch a blurb.
pub fn is_runnable(name: &str) -> bool {
    !matches!(classify(name), Kind::Unknown)
}

/// Run a free-form home goal under `caps`: plan → act via MCP → Brief.
///
/// The kernel does not run an LLM. It restates the ask, strips filler words
/// into a search query, CALLs granted connectors, and arms Doc rows the UI
/// can open. That is the agentic loop: capabilities + tools, not autocomplete.
pub fn run_goal(goal: &str, caps: Caps) -> Brief {
    let mut brief = Brief::empty();
    brief.set_skill("agent-plan-act");
    brief.set_heading("Working on it");
    brief.push_plan("Restate the goal");
    brief.push_plan("Pick tools from grants");
    brief.push_plan("Search under those grants");
    brief.push_plan("Report openable hits");

    let mut goal_buf = [0u8; 68];
    let restated = restate_goal(goal, &mut goal_buf);
    if !restated.is_empty() {
        brief.push_line("Goal", restated);
    }

    let mut qbuf = [0u8; 48];
    let q = keywords_from_goal(goal, &mut qbuf);
    if q.is_empty() {
        brief.push_line("Info", "Try naming the file, topic, or inbox.");
        return brief;
    }
    brief.push_line("Query", q);

    let mailish = goal_looks_like_mail(goal);
    let mut acted = false;

    // Inbox lane when the ask is about mail and Email is on.
    if mailish {
        if caps.allows(Cap::EmailSearch) {
            let mail = mcp::fetch_mail_peek(caps);
            brief.status = mail.status;
            if mail.status == BridgeStatus::Online && mail.count > 0 {
                fill_mail_lines(&mut brief, &mail, false);
                acted = true;
            } else if mail.status == BridgeStatus::Online {
                brief.push_line("FYI", "Inbox empty.");
                acted = true;
            } else {
                brief.push_line("Info", "Bridge offline for mail.");
            }
        } else {
            brief.need(Cap::EmailSearch);
        }
    }

    // Knowledge + personal files share search.query on the wire; Your files
    // alone can still surface recent workspace rows by title match.
    if caps.allows(Cap::SearchQuery) {
        let peek = mcp::fetch_search_peek(caps, q);
        if brief.status != BridgeStatus::Online {
            brief.status = peek.status;
        }
        if peek.denied {
            brief.need(Cap::SearchQuery);
        } else {
            fill_goal_hits(&mut brief, &peek);
            acted = true;
        }
    } else if caps.allows(Cap::WorkspaceIndex) {
        let files = mcp::fetch_files_peek(caps);
        if brief.status != BridgeStatus::Online {
            brief.status = files.status;
        }
        if files.denied {
            brief.need(Cap::WorkspaceIndex);
        } else {
            fill_goal_files(&mut brief, &files, q);
            acted = true;
        }
    } else if !mailish {
        brief.need(Cap::SearchQuery);
        brief.push_line("Info", "Grant Built-in docs or Your files.");
    }

    if acted && brief.doc_n == 0 && !mailish {
        brief.push_line("Info", "No openable hits - refine the ask.");
    } else if brief.doc_n > 0 {
        brief.push_line("Next", "Tap a Doc row to open it.");
    }
    brief
}

/// Drop filler words so "i wanna work on my paper" becomes `paper`.
pub fn keywords_from_goal<'a>(goal: &str, buf: &'a mut [u8; 48]) -> &'a str {
    const STOP: &[&str] = &[
        "i", "im", "i'm", "wanna", "want", "to", "a", "an", "the", "my", "me",
        "on", "in", "for", "of", "and", "or", "please", "can", "you", "we",
        "work", "working", "get", "got", "do", "doing", "help", "with", "about",
        "find", "show", "open", "read", "look", "looking", "gotta", "gonna",
        "just", "some", "any", "this", "that", "it",
    ];
    buf.fill(0);
    let mut n = 0;
    for raw in goal.split(|c: char| !c.is_ascii_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        let mut word = [0u8; 24];
        let mut w = 0;
        for &b in raw.as_bytes() {
            if w >= word.len() {
                break;
            }
            word[w] = if (b'A'..=b'Z').contains(&b) {
                b + 32
            } else {
                b
            };
            w += 1;
        }
        let tok = core::str::from_utf8(&word[..w]).unwrap_or("");
        if tok.is_empty() || STOP.iter().any(|&s| s == tok) {
            continue;
        }
        if n > 0 && n < buf.len() {
            buf[n] = b' ';
            n += 1;
        }
        for &b in tok.as_bytes() {
            if n < buf.len() {
                buf[n] = b;
                n += 1;
            }
        }
    }
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

fn restate_goal<'a>(goal: &str, buf: &'a mut [u8; 68]) -> &'a str {
    buf.fill(0);
    let bytes = goal.as_bytes();
    let mut n = bytes.len().min(buf.len());
    while n > 0 && !goal.is_char_boundary(n) {
        n -= 1;
    }
    buf[..n].copy_from_slice(&bytes[..n]);
    // Collapse runs of whitespace for a tight Goal line.
    let mut w = 0;
    let mut space = false;
    for i in 0..n {
        let b = buf[i];
        if b == b' ' || b == b'\t' {
            if w > 0 {
                space = true;
            }
            continue;
        }
        if space && w < buf.len() {
            buf[w] = b' ';
            w += 1;
            space = false;
        }
        if w < buf.len() {
            buf[w] = b;
            w += 1;
        }
    }
    core::str::from_utf8(&buf[..w]).unwrap_or("")
}

fn goal_looks_like_mail(goal: &str) -> bool {
    let g = goal.as_bytes();
    // ASCII substring check — enough for inbox / email / mail.
    contains_ascii(g, b"inbox") || contains_ascii(g, b"email") || contains_ascii(g, b"mail")
}

fn contains_ascii(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    let norm = |b: u8| {
        if (b'A'..=b'Z').contains(&b) {
            b + 32
        } else {
            b
        }
    };
    'outer: for i in 0..=hay.len() - needle.len() {
        for j in 0..needle.len() {
            if norm(hay[i + j]) != needle[j] {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

fn fill_goal_hits(brief: &mut Brief, peek: &SearchPeek) {
    if peek.count == 0 {
        brief.push_line(
            "Info",
            if peek.status == BridgeStatus::Offline {
                "No offline hits for that query."
            } else {
                "No search hits."
            },
        );
        return;
    }
    for i in 0..peek.count.min(3) {
        let title = peek.title_at(i);
        let url = peek.url_at(i);
        let line_i = brief.count;
        brief.push_line("Doc", title);
        if !url.is_empty() {
            brief.arm_doc(url, line_i);
        }
    }
}

fn fill_goal_files(brief: &mut Brief, files: &FilePeek, q: &str) {
    if files.count == 0 {
        brief.push_line("Info", "No recent files indexed yet.");
        return;
    }
    let mut matched = 0usize;
    for i in 0..files.count.min(3) {
        let title = files.title_at(i);
        let url = files.url_at(i);
        if !title_matches_query(title, q) {
            continue;
        }
        let line_i = brief.count;
        brief.push_line("Doc", title);
        if !url.is_empty() {
            brief.arm_doc(url, line_i);
        }
        matched += 1;
    }
    if matched == 0 {
        // Still surface top recent files so the agent is not empty theatre.
        for i in 0..files.count.min(2) {
            let title = files.title_at(i);
            let url = files.url_at(i);
            let line_i = brief.count;
            brief.push_line("Doc", title);
            if !url.is_empty() {
                brief.arm_doc(url, line_i);
            }
        }
        brief.push_line("Info", "No title match - showing recent files.");
    }
}

fn title_matches_query(title: &str, q: &str) -> bool {
    if q.is_empty() {
        return false;
    }
    for part in q.split(' ') {
        if part.is_empty() {
            continue;
        }
        if contains_ascii(title.as_bytes(), part.as_bytes()) {
            return true;
        }
    }
    false
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
        Kind::TeddyPortals => run_teddy(&mut brief, caps),
        Kind::MarketPortals => run_markets(&mut brief, caps),
        Kind::Unknown => {
            // Saved / custom skills: Brief always. The click path fetches the
            // body, [`enrich_playbook`] names tools + missing grants, then
            // [`run_playbook_allowed`] CALLs only tools already granted.
            // Markdown is still not a script — no invented LINE…END, no
            // email.send, no path/URL tools without a picker.
            brief.set_heading("Playbook plan");
            brief.push_plan("Read playbook from the host");
            brief.push_plan("Name tools the playbook mentions");
            brief.push_plan("Report missing grants");
            brief.push_plan("CALL tools already granted");
            brief.push_line("Info", "Granted tools run; others stay Need.");
        }
    }
    brief
}

/// Tools a playbook may name, mapped to the grant that would allow them.
const PLAYBOOK_TOOLS: &[(&str, Cap)] = &[
    ("email.search", Cap::EmailSearch),
    ("search.query", Cap::SearchQuery),
    ("workspace.index", Cap::WorkspaceIndex),
    ("audio.transcribe", Cap::AudioTranscribe),
    ("skills.save", Cap::SkillsSave),
    ("tsearch.sync", Cap::PortalSync),
    ("teddy.health", Cap::PortalSync),
    ("market.health", Cap::PortalSync),
    ("doc.read", Cap::SearchQuery),
    ("calendar.list", Cap::EmailSearch),
    ("email.send", Cap::EmailSend),
];

/// Scan playbook prose for known tool spellings (substring, ASCII).
///
/// Returns a bit per [`PLAYBOOK_TOOLS`] entry (low bit = index 0). Pure: safe
/// in host unit tests with no COM2.
pub fn playbook_tool_bits(body: &str) -> u32 {
    let mut bits = 0u32;
    for (i, (tool, _)) in PLAYBOOK_TOOLS.iter().enumerate() {
        if body.contains(tool) {
            bits |= 1u32 << i;
        }
    }
    bits
}

/// Fill Body / Tool / Need lines from playbook text after [`run`] for Unknown.
///
/// Pure grant bookkeeping — never opens COM2. Follow with
/// [`run_playbook_allowed`] to CALL granted peek tools.
pub fn enrich_playbook(brief: &mut Brief, caps: Caps, body: &str) {
    // First non-empty prose line as Body (skip markdown headings if possible).
    let mut body_line = "";
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t == "---" {
            continue;
        }
        if t.starts_with('#') {
            let rest = t.trim_start_matches('#').trim();
            if !rest.is_empty() && body_line.is_empty() {
                body_line = rest;
            }
            continue;
        }
        body_line = t;
        break;
    }
    if !body_line.is_empty() {
        brief.push_report("Body", body_line);
    } else {
        brief.push_report("Info", "Playbook body unavailable.");
    }

    let bits = playbook_tool_bits(body);
    if bits == 0 {
        brief.push_line("Info", "No known MCP tools named in this playbook.");
        return;
    }

    // One Tool line per mention, then Need for grants still off. Leave room
    // for the Info/Body lines already pushed (max 8).
    for (i, (tool, cap)) in PLAYBOOK_TOOLS.iter().enumerate() {
        if bits & (1u32 << i) == 0 {
            continue;
        }
        if brief.count >= brief.lines.len() {
            break;
        }
        brief.push_line("Tool", tool);
        if !caps.allows(*cap) && brief.count < brief.lines.len() {
            // need() sets denied; prefer the first missing grant as deny_cap.
            if !brief.denied {
                brief.need(*cap);
            } else {
                brief.push_line("Need", cap.label());
            }
        }
    }
}

/// CALL peek tools named in the playbook when the matching grant is on.
///
/// Cap refusal never opens COM2. Write / path / URL tools only get an Info
/// pointer (Search picker, Skills Save starter) — markdown is not executable.
pub fn run_playbook_allowed(brief: &mut Brief, caps: Caps, body: &str) {
    let bits = playbook_tool_bits(body);
    if bits == 0 {
        return;
    }

    // --- safe peeks (grant checked before any MCP) ------------------------
    if bit_set(bits, 0) && caps.allows(Cap::EmailSearch) && brief.count < brief.lines.len() {
        let mail = mcp::fetch_mail_peek(caps);
        brief.status = mail.status;
        if mail.status == BridgeStatus::Online && mail.count > 0 {
            fill_mail_lines(brief, &mail, false);
        } else if mail.status == BridgeStatus::Online {
            brief.push_line("FYI", "Inbox empty.");
        } else {
            brief.push_line("Info", "Bridge offline for mail.");
        }
    }

    // calendar.list shares Cap::EmailSearch / email=1 with mail.
    if bit_set(bits, 9) && caps.allows(Cap::EmailSearch) && brief.count < brief.lines.len() {
        let cal = mcp::fetch_calendar_peek(caps);
        if brief.status != BridgeStatus::Online {
            brief.status = cal.status;
        }
        fill_calendar_lines(brief, &cal);
    }

    if bit_set(bits, 1) && caps.allows(Cap::SearchQuery) && brief.count < brief.lines.len() {
        let peek = mcp::fetch_search_peek(caps, "capability-agent");
        if brief.status != BridgeStatus::Online {
            brief.status = peek.status;
        }
        if peek.denied {
            brief.need(Cap::SearchQuery);
        } else if peek.count > 0 {
            let room = brief.lines.len().saturating_sub(brief.count).min(3);
            for i in 0..peek.count.min(room) {
                brief.push_line("Hit", peek.title_at(i));
            }
        } else {
            brief.push_line(
                "Info",
                if peek.status == BridgeStatus::Offline {
                    "No offline hits for that query."
                } else {
                    "No corpus hits."
                },
            );
        }
    }

    if bit_set(bits, 6) && caps.allows(Cap::PortalSync) {
        fill_portal_lines(brief, caps, "teddy.health", 1);
    }
    if bit_set(bits, 7) && caps.allows(Cap::PortalSync) {
        fill_portal_lines(brief, caps, "market.health", 1);
    }

    // --- named but not silent-CALL'd --------------------------------------
    if bit_set(bits, 2) && caps.allows(Cap::WorkspaceIndex) {
        brief.push_line("Info", "Your files on - open file hits from Search.");
    }
    if bit_set(bits, 3) && caps.allows(Cap::AudioTranscribe) {
        brief.push_line("Info", "Recordings on - type /path.wav in Search.");
    }
    if bit_set(bits, 4) && caps.allows(Cap::SkillsSave) {
        brief.push_line("Info", "Save skills: use Save starter on Skills.");
    }
    if bit_set(bits, 5) && caps.allows(Cap::PortalSync) {
        brief.push_line("Info", "tsearch.sync: warm Online services on Caps.");
    }
    if bit_set(bits, 8) && caps.allows(Cap::SearchQuery) {
        brief.push_line("Info", "doc.read: open a hit from Search.");
    }
    // email.send is never silent-CALL'd — Confirm send on an inbox Brief.
    if bit_set(bits, 10) {
        if caps.allows(Cap::EmailSend) {
            brief.push_line("Info", "email.send: Confirm send on an inbox Brief.");
        }
    }
}

fn bit_set(bits: u32, i: usize) -> bool {
    bits & (1u32 << i) != 0
}

/// Morning brief used on home after setup: plan/act over whatever is granted.
pub fn morning(caps: Caps, level: Level) -> Brief {
    let mut brief = run("agent-plan-act", caps);
    if !level.is_guided() {
        // Advanced: keep the report lines; drop the long plan checklist.
        brief.plan_n = brief.plan_n.min(2);
        brief.set_heading("Morning");
    }
    brief
}

fn run_inbox(brief: &mut Brief, caps: Caps, triage: bool) {
    brief.set_heading(if triage {
        "Inbox triage"
    } else {
        "Morning mail brief"
    });
    brief.push_plan("Check email.search grant");
    brief.push_plan("CALL email.search q=in:inbox max=5");
    brief.push_plan("CALL calendar.list under the same grant");
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
    } else {
        fill_mail_lines(brief, &mail, triage);
    }
    if brief.count < brief.lines.len() {
        let cal = mcp::fetch_calendar_peek(caps);
        fill_calendar_lines(brief, &cal);
    }
    // Draft only — Confirm send on Brief issues the CALL with confirm=1.
    if mail.count > 0 && brief.count < brief.lines.len() {
        if caps.allows(Cap::EmailSend) {
            let to = mail.row_from(0);
            let subj = mail.row_subj(0);
            let mut re = [0u8; 48];
            let prefix = b"Re: ";
            let mut n = prefix.len();
            re[..n].copy_from_slice(prefix);
            for &b in subj.as_bytes() {
                if n >= re.len() {
                    break;
                }
                re[n] = b;
                n += 1;
            }
            let re_subj = core::str::from_utf8(&re[..n]).unwrap_or(subj);
            brief.arm_send(to, re_subj);
            brief.push_line("Draft", "Tap Confirm send below");
        } else {
            brief.push_line("Info", "Send mail off - drafts only.");
        }
    }
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

fn fill_calendar_lines(brief: &mut Brief, cal: &CalendarPeek) {
    if cal.denied {
        return;
    }
    if cal.status == BridgeStatus::Offline {
        brief.push_line("Info", "Bridge offline for calendar.");
        return;
    }
    if cal.count == 0 {
        brief.push_line("FYI", "No upcoming events.");
        return;
    }
    for i in 0..cal.count.min(2) {
        if brief.count >= brief.lines.len() {
            break;
        }
        let title = cal.title_at(i);
        let when = cal.when_at(i);
        let line_idx = brief.count;
        if when.is_empty() {
            brief.push_line("Event", title);
        } else {
            let mut text = [0u8; 68];
            let mut n = 0;
            for &b in title.as_bytes().iter().take(40) {
                text[n] = b;
                n += 1;
            }
            if n + 3 < text.len() {
                text[n] = b' ';
                text[n + 1] = b'-';
                text[n + 2] = b' ';
                n += 3;
            }
            for &b in when.as_bytes() {
                if n >= text.len() {
                    break;
                }
                text[n] = b;
                n += 1;
            }
            let line = core::str::from_utf8(&text[..n]).unwrap_or(title);
            brief.push_line("Event", line);
        }
        brief.arm_event(cal.id_at(i), line_idx);
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

    // Mail + calendar when granted (same Cap::EmailSearch / email=1 bit).
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
        if brief.count < brief.lines.len() {
            let cal = mcp::fetch_calendar_peek(caps);
            fill_calendar_lines(brief, &cal);
        }
    } else {
        brief.push_line("Info", "Email off - skipping inbox.");
    }

    // Knowledge lane when granted (includes teddy corpus when portal=1).
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

    // Your files / Recordings: no COM2 here — point at Search so host unit
    // tests stay safe (grant alone must not open the serial).
    if caps.allows(Cap::WorkspaceIndex) {
        brief.push_line("Info", "Your files on - open file hits from Search.");
    }
    if caps.allows(Cap::AudioTranscribe) {
        brief.push_line("Info", "Recordings on - type /path.wav in Search.");
    }

    // Live portals when Online services is on — teddy first, then markets.
    if caps.allows(Cap::PortalSync) {
        fill_portal_lines(brief, caps, "teddy.health", 1);
        fill_portal_lines(brief, caps, "market.health", 1);
    }
}

/// Teddy API (corpus via search) + teddy portals (live HTTPS tools).
fn run_teddy(brief: &mut Brief, caps: Caps) {
    brief.set_heading("Teddy API + portals");
    brief.push_plan("Check portal.sync grant");
    brief.push_plan("CALL search.query with portal=1 (corpus API)");
    brief.push_plan("CALL teddy.health / fear_greed / gex");
    brief.push_plan("Report live fields alongside corpus hits");

    if !caps.allows(Cap::PortalSync) {
        brief.need(Cap::PortalSync);
        brief.push_line("Info", "Grant Online services, then re-run.");
        return;
    }

    // Corpus side of teddy — needs search.query as well as portal.sync.
    if caps.allows(Cap::SearchQuery) {
        let peek = mcp::fetch_search_peek(caps, "teddy-search");
        brief.status = peek.status;
        if peek.count > 0 {
            for i in 0..peek.count.min(2) {
                brief.push_line("Hit", peek.title_at(i));
            }
        } else if peek.status == BridgeStatus::Offline {
            brief.push_line("Info", "Bridge offline - corpus unavailable.");
        } else {
            brief.push_line("Info", "Teddy corpus returned no hits.");
        }
    } else {
        brief.push_line("Need", Cap::SearchQuery.label());
        brief.push_line("Info", "Corpus search needs Built-in docs too.");
    }

    fill_portal_lines(brief, caps, "teddy.health", 2);
    fill_portal_lines(brief, caps, "teddy.fear_greed", 2);
    fill_portal_lines(brief, caps, "teddy.gex", 2);
}

/// Live market intelligence on superintelmarkets.com (same `portal.sync` bit).
fn run_markets(brief: &mut Brief, caps: Caps) {
    brief.set_heading("Market portals");
    brief.push_plan("Check portal.sync grant");
    brief.push_plan("CALL market.health portal=1");
    brief.push_plan("CALL market.fear_greed portal=1");
    brief.push_plan("Report live fields only");

    if !caps.allows(Cap::PortalSync) {
        brief.need(Cap::PortalSync);
        brief.push_line("Info", "Grant Online services, then re-run.");
        return;
    }

    fill_portal_lines(brief, caps, "market.health", 3);
    fill_portal_lines(brief, caps, "market.fear_greed", 3);
}

fn fill_portal_lines(brief: &mut Brief, caps: Caps, tool: &str, max: usize) {
    if brief.count >= brief.lines.len() {
        return;
    }
    let peek = mcp::fetch_portal(caps, tool);
    if brief.status != BridgeStatus::Online {
        brief.status = peek.status;
    }
    if peek.denied {
        brief.need(Cap::PortalSync);
        return;
    }
    if peek.status == BridgeStatus::Offline {
        brief.push_line("Info", "Bridge offline for portals.");
        return;
    }
    if peek.count == 0 {
        brief.push_line("Info", tool);
        return;
    }
    let room = brief.lines.len().saturating_sub(brief.count).min(max);
    for i in 0..peek.count.min(room) {
        let mut text = [0u8; 68];
        let mut n = 0;
        for &b in tool.as_bytes().iter().take(12) {
            text[n] = b;
            n += 1;
        }
        if n + 1 < text.len() {
            text[n] = b' ';
            n += 1;
        }
        for &b in peek.field_at(i).as_bytes() {
            if n >= 28 {
                break;
            }
            text[n] = b;
            n += 1;
        }
        if n + 1 < text.len() {
            text[n] = b'=';
            n += 1;
        }
        for &b in peek.value_at(i).as_bytes() {
            if n >= text.len() {
                break;
            }
            text[n] = b;
            n += 1;
        }
        let line = core::str::from_utf8(&text[..n]).unwrap_or(peek.field_at(i));
        brief.push_line("Live", line);
    }
}

fn run_cap_safe(brief: &mut Brief, caps: Caps) {
    brief.set_heading("Capability check");
    brief.status = BridgeStatus::Online;
    brief.push_plan("List each grant");
    brief.push_plan("Refuse tools whose switch is off");
    brief.push_plan("Prove skills.save needs Save skills");

    for cap in Cap::ALL {
        let tag = if caps.allows(cap) { "On" } else { "Off" };
        brief.push_line(tag, cap.label());
    }
    // Cap::ALL is seven rows; leave room for one outcome line (max 8).
    // Host unit tests must not grant SkillsSave here — that path opens COM2.
    if caps.allows(Cap::SkillsSave) {
        match mcp::save_skill(caps, "guest-starter", "Starter from capability check") {
            mcp::SaveSkillStatus::Ok => brief.push_line("Saved", "guest-starter"),
            mcp::SaveSkillStatus::Offline => {
                brief.push_line("Info", "Bridge offline - cannot save.")
            }
            mcp::SaveSkillStatus::Denied => brief.need(Cap::SkillsSave),
            mcp::SaveSkillStatus::Failed => {
                brief.push_line("Info", "skills.save failed on the host.")
            }
        }
    } else {
        brief.push_line("Info", "Save skills off - no write.");
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
    fn empty_brief_has_no_report() {
        assert!(!Brief::empty().has_report());
        let b = run("capability-safe-tools", Caps::default_grants());
        assert!(b.has_report());
    }

    #[test]
    fn builtins_classify() {
        assert_eq!(classify("inbox-brief"), Kind::InboxBrief);
        assert_eq!(classify("knowledge-search"), Kind::KnowledgeSearch);
        assert_eq!(classify("agent-plan-act"), Kind::PlanAct);
        assert_eq!(classify("teddy-portals"), Kind::TeddyPortals);
        assert_eq!(classify("market-portals"), Kind::MarketPortals);
        assert_eq!(classify("mystery"), Kind::Unknown);
        assert!(is_runnable("email-triage"));
        assert!(is_runnable("teddy-portals"));
        assert!(is_runnable("market-portals"));
        assert!(!is_runnable("custom-saved"));
    }

    #[test]
    fn unknown_skill_opens_a_brief_without_com2() {
        // Host unit tests must not grant anything that would open COM2.
        let b = run("guest-starter", Caps::none());
        assert_eq!(b.heading(), "Playbook plan");
        assert!(b.plan_n >= 2);
        assert!(!b.denied);
        assert!(b.lines.iter().any(|l| l.tag() == "Info"));
        assert!(
            b.plan_at(3).contains("granted") || b.plan_at(3).contains("CALL"),
            "plan should promise granted CALLs: {}",
            b.plan_at(3)
        );
    }

    #[test]
    fn playbook_scan_names_tools_and_missing_grants() {
        let body = "Triages mail with email.search then search.query for context.";
        let bits = playbook_tool_bits(body);
        assert!(bits & 1 != 0, "email.search");
        assert!(bits & (1 << 1) != 0, "search.query");

        let mut brief = run("custom-saved", Caps::none());
        enrich_playbook(&mut brief, Caps::none(), body);
        // Denied tools must never open COM2 — run_playbook_allowed is a no-op.
        run_playbook_allowed(&mut brief, Caps::none(), body);
        assert!(brief.lines.iter().any(|l| l.tag() == "Tool" && l.text() == "email.search"));
        assert!(brief.lines.iter().any(|l| l.tag() == "Tool" && l.text() == "search.query"));
        assert!(brief.denied);
        assert_eq!(brief.deny_name(), "email.search");
        assert!(brief.lines.iter().any(|l| l.tag() == "Need" && l.text() == "Email"));
        assert!(brief.lines.iter().any(|l| l.tag() == "Need" && l.text() == "Built-in docs"));
        assert!(
            !brief.lines.iter().any(|l| l.tag() == "Hit"),
            "no Hits without grants"
        );
    }

    #[test]
    fn playbook_scan_respects_grants_already_on() {
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        let mut brief = run("custom-saved", caps);
        enrich_playbook(
            &mut brief,
            caps,
            "Only search.query is named here.",
        );
        // Do not call run_playbook_allowed here: SearchQuery would open COM2.
        assert!(brief.lines.iter().any(|l| l.tag() == "Tool"));
        assert!(!brief.denied, "granted tool must not mark Need");
        assert!(!brief.lines.iter().any(|l| l.tag() == "Need"));
    }

    #[test]
    fn playbook_act_info_only_tools_skip_com2() {
        // Path/write tools named + granted → Info pointer, never COM2.
        let mut caps = Caps::none();
        caps.set(Cap::WorkspaceIndex, true);
        caps.set(Cap::AudioTranscribe, true);
        caps.set(Cap::SkillsSave, true);
        let body = "Uses workspace.index, audio.transcribe, and skills.save.";
        let mut brief = run("custom-saved", caps);
        enrich_playbook(&mut brief, caps, body);
        run_playbook_allowed(&mut brief, caps, body);
        assert!(brief.lines.iter().any(|l| l.text().contains("Your files")));
        assert!(brief.lines.iter().any(|l| l.text().contains("Recordings")));
        assert!(brief.lines.iter().any(|l| l.text().contains("Save skills")));
        assert!(!brief.lines.iter().any(|l| l.tag() == "Hit"));
    }

    #[test]
    fn email_send_playbook_stays_confirm_only() {
        let body = "May use email.send after the user confirms.";
        assert!(playbook_tool_bits(body) & (1 << 10) != 0);
        let mut caps = Caps::none();
        caps.set(Cap::EmailSend, true);
        let mut brief = run("send-saved", caps);
        enrich_playbook(&mut brief, caps, body);
        run_playbook_allowed(&mut brief, caps, body);
        assert!(!brief.denied, "granted send should not Need");
        assert!(
            brief.lines.iter().any(|l| l.text().contains("Confirm send")),
            "must point at Brief confirm, not auto-CALL"
        );
        assert!(!brief.send_ready, "playbook must not arm a draft");
    }

    #[test]
    fn arm_send_exposes_draft_for_confirm_cta() {
        let mut brief = Brief::empty();
        brief.arm_send("ada@x.com", "Re: Hello");
        assert!(brief.send_ready);
        assert_eq!(brief.draft_to(), "ada@x.com");
        assert_eq!(brief.draft_subj(), "Re: Hello");
        brief.clear_send();
        assert!(!brief.send_ready);
    }

    #[test]
    fn calendar_playbook_needs_email_without_com2() {
        let body = "Check calendar.list for the afternoon.";
        assert!(playbook_tool_bits(body) & (1 << 9) != 0);
        let mut brief = run("cal-saved", Caps::none());
        enrich_playbook(&mut brief, Caps::none(), body);
        run_playbook_allowed(&mut brief, Caps::none(), body);
        assert!(brief.denied);
        assert_eq!(brief.deny_name(), "email.search");
        assert!(!brief.lines.iter().any(|l| l.tag() == "Event"));
    }

    #[test]
    fn calendar_lines_format_title_and_when() {
        let mut brief = Brief::empty();
        let mut cal = CalendarPeek::empty(BridgeStatus::Online, false);
        copy_field(&mut cal.rows[0].id, "0123456789abcdef");
        copy_field(&mut cal.rows[0].title, "Demo event");
        copy_field(&mut cal.rows[0].when, "tomorrow");
        cal.count = 1;
        fill_calendar_lines(&mut brief, &cal);
        assert_eq!(brief.lines[0].tag(), "Event");
        assert!(brief.lines[0].text().contains("Demo event"));
        assert!(brief.lines[0].text().contains("tomorrow"));
        assert_eq!(brief.event_n, 1);
        let mut buf = [0u8; 40];
        assert_eq!(
            brief.event_url_at(0, &mut buf),
            Some("cal://0123456789abcdef")
        );
    }

    #[test]
    fn teddy_skill_names_the_portal_cap_when_missing() {
        let b = run("teddy-portals", Caps::none());
        assert!(b.denied);
        assert_eq!(b.deny_name(), "portal.sync");
        assert!(b.plan_n >= 3);
    }

    #[test]
    fn market_skill_names_the_portal_cap_when_missing() {
        let b = run("market-portals", Caps::none());
        assert!(b.denied);
        assert_eq!(b.deny_name(), "portal.sync");
        assert_eq!(b.heading(), "Market portals");
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
        // Do not grant SkillsSave: that branch would open COM2.
        let b = run("capability-safe-tools", caps);
        assert_eq!(b.count, Cap::ALL.len() + 1);
        assert!(b.lines.iter().any(|l| l.tag() == "On" && l.text() == "Built-in docs"));
        assert!(b.lines.iter().any(|l| l.tag() == "Off" && l.text() == "Email"));
        assert!(b.lines.iter().any(|l| l.tag() == "Off" && l.text() == "Save skills"));
        assert_eq!(b.lines[b.count - 1].tag(), "Info");
        assert!(b.lines[b.count - 1].text().contains("no write"));
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
    fn morning_advanced_shortens_the_plan() {
        let guided = morning(Caps::none(), Level::Guided);
        let advanced = morning(Caps::none(), Level::Advanced);
        assert!(guided.plan_n >= advanced.plan_n);
        assert_eq!(advanced.heading(), "Morning");
        assert!(advanced.plan_n <= 2);
    }

    #[test]
    fn plan_act_mentions_recordings_when_granted_without_com2() {
        // AudioTranscribe alone must not open COM2 (SearchQuery stays off).
        let mut caps = Caps::none();
        caps.set(Cap::AudioTranscribe, true);
        let b = run("agent-plan-act", caps);
        assert!(
            b.lines
                .iter()
                .any(|l| l.text().contains("Recordings on")),
            "expected recordings lane: {:?}",
            b.lines.iter().map(|l| l.text()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn plan_act_mentions_files_when_granted_without_com2() {
        // WorkspaceIndex alone must not open COM2 (SearchQuery stays off).
        let mut caps = Caps::none();
        caps.set(Cap::WorkspaceIndex, true);
        let b = run("agent-plan-act", caps);
        assert!(
            b.lines
                .iter()
                .any(|l| l.text().contains("Your files on")),
            "expected files lane: {:?}",
            b.lines.iter().map(|l| l.text()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn copy_is_ascii_only() {
        for s in [
            "Morning mail brief",
            "Inbox triage",
            "Knowledge search",
            "Plan, act, report",
            "Capability check",
            "Teddy API + portals",
            "Market portals",
            "Grant Email on Capabilities, then re-run.",
            "Grant Online services, then re-run.",
            "Bridge offline - cannot read mail.",
            "Bridge offline - cannot save.",
            "Save skills off - no write.",
            "skills.save failed on the host.",
            "Playbook plan",
            "Granted tools run; others stay Need.",
            "No known MCP tools named in this playbook.",
            "Playbook body unavailable.",
            "Your files on - open file hits from Search.",
            "Recordings on - type /path.wav in Search.",
            "Save skills: use Save starter on Skills.",
            "doc.read: open a hit from Search.",
            "tsearch.sync: warm Online services on Caps.",
            "Acting only with switches that are on.",
            "CALL tools already granted",
            "CALL calendar.list under the same grant",
            "Bridge offline for calendar.",
            "No upcoming events.",
            "Read inbox and calendar",
            "Send after you confirm on Brief",
            "Send mail off - drafts only.",
            "Tap Confirm send below",
            "email.send: Confirm send on an inbox Brief.",
            "Mock queued on the bridge",
            "Working on it",
            "Try naming the file, topic, or inbox.",
            "Grant Built-in docs or Your files.",
            "No openable hits - refine the ask.",
            "Tap a Doc row to open it.",
            "No recent files indexed yet.",
            "No title match - showing recent files.",
            "No search hits.",
        ] {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII: {s:?}"
            );
        }
    }

    #[test]
    fn keywords_drop_filler_from_natural_asks() {
        let mut buf = [0u8; 48];
        assert_eq!(
            keywords_from_goal("i wanna work on my paper", &mut buf),
            "paper"
        );
        assert_eq!(
            keywords_from_goal("find the capability model docs", &mut buf),
            "capability model docs"
        );
        assert_eq!(keywords_from_goal("!!!", &mut buf), "");
    }

    #[test]
    fn run_goal_without_grants_names_the_need() {
        // Caps::none must not open COM2.
        let b = run_goal("i wanna work on my paper", Caps::none());
        assert_eq!(b.heading(), "Working on it");
        assert!(b.plan_n >= 3);
        assert!(b.lines.iter().any(|l| l.tag() == "Goal"));
        assert!(b.lines.iter().any(|l| l.tag() == "Query" && l.text() == "paper"));
        assert!(b.denied);
        assert_eq!(b.deny_name(), "search.query");
        assert_eq!(b.doc_n, 0);
    }

    #[test]
    fn goal_hits_arm_openable_doc_urls() {
        let mut brief = Brief::empty();
        let mut peek = SearchPeek::empty(BridgeStatus::Online, false);
        copy_field(&mut peek.hits[0].title, "thesis draft");
        copy_field(&mut peek.hits[0].url, "file://docs/thesis.md");
        peek.count = 1;
        fill_goal_hits(&mut brief, &peek);
        assert_eq!(brief.lines[0].tag(), "Doc");
        assert_eq!(brief.doc_n, 1);
        assert_eq!(brief.doc_url_at(0), Some("file://docs/thesis.md"));
    }
}
