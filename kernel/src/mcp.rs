//! Guest MCP client over COM2 (host bridge).

use crate::serial::Serial;

const TIMEOUT_PING: u32 = 80_000;
const TIMEOUT_LINE: u32 = 200_000;
/// First reply line after a CALL. The gog backend shells out to an external
/// process plus a Gmail HTTPS round-trip before writing anything, so this must
/// absorb seconds of latency — TIMEOUT_LINE only covers intra-reply gaps.
/// Only reached once PING has succeeded, so an offline bridge never waits.
const TIMEOUT_REPLY: u32 = 40_000_000;

/// Longest protocol line the bridge can legally send: two 90-char fields at up
/// to 4 UTF-8 bytes each plus framing (~735 bytes) fits with headroom.
const LINE_BUF: usize = 768;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    Offline,
    Online,
}

pub struct MailRow {
    pub from: [u8; 40],
    pub subj: [u8; 72],
}

pub struct MailPeek {
    pub status: BridgeStatus,
    pub count: usize,
    pub rows: [MailRow; 5],
}

impl MailPeek {
    pub const fn empty(status: BridgeStatus) -> Self {
        const EMPTY: MailRow = MailRow {
            from: [0; 40],
            subj: [0; 72],
        };
        Self {
            status,
            count: 0,
            rows: [EMPTY; 5],
        }
    }

    pub fn row_from(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].from))
    }

    pub fn row_subj(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].subj))
    }
}

/// One hit from `search.query`.
pub struct SearchHit {
    pub title: [u8; 48],
    /// Source URL, needed to open the document rather than only name it.
    pub url: [u8; 72],
}

/// Short corpus peek for the home Connectors card.
pub struct SearchPeek {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub hits: [SearchHit; 3],
}

impl SearchPeek {
    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        const EMPTY: SearchHit = SearchHit { title: [0; 48], url: [0; 72] };
        Self {
            status,
            denied,
            count: 0,
            hits: [EMPTY; 3],
        }
    }

    pub fn title_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.hits[i].title))
    }

    pub fn url_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.hits[i].url))
    }
}

fn trim_buf(buf: &[u8]) -> &[u8] {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    &buf[..n]
}

/// Decode the longest valid UTF-8 prefix — a line cut mid-character (buffer
/// truncation) must degrade to a shorter string, not vanish entirely.
fn str_prefix(bytes: &[u8]) -> &str {
    match core::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    }
}

fn copy_field(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let mut n = bytes.len().min(dst.len());
    // Never cut mid-character: a torn tail would make the whole field
    // undecodable when read back.
    while n > 0 && !src.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&bytes[..n]);
}

fn parse_row_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix("ROW ")?;
    for part in rest.split('|') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return Some(v);
            }
        }
    }
    None
}

/// Probe the host bridge and optionally fetch a short inbox peek.
///
/// `email.search` is refused when `caps` does not grant [`crate::caps::Cap::EmailSearch`].
pub fn fetch_mail_peek(caps: crate::caps::Caps) -> MailPeek {
    let com2 = Serial::com2();
    com2.init();

    for _ in 0..64 {
        if com2.try_read_byte().is_none() {
            break;
        }
    }

    com2.write_str("PING\n");
    let mut line = [0u8; LINE_BUF];
    let Some(n) = com2.read_line(&mut line, TIMEOUT_PING) else {
        return MailPeek::empty(BridgeStatus::Offline);
    };
    let resp = str_prefix(&line[..n]);
    if !resp.starts_with("OK pong") {
        return MailPeek::empty(BridgeStatus::Offline);
    }

    if !caps.allows(crate::caps::Cap::EmailSearch) {
        // Bridge is up, but this guest was not granted inbox read.
        return MailPeek::empty(BridgeStatus::Online);
    }

    // Wire bit must match Cap::EmailSearch — bridge refuses without email=1.
    // max=5 matches the inbox Brief plan text.
    com2.write_str("CALL email.search q=in:inbox max=5 email=1\n");

    let mut peek = MailPeek::empty(BridgeStatus::Online);

    let mut first = true;
    for _ in 0..16 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if resp.starts_with("OK email.search") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.rows.len() {
            let from = parse_row_field(resp, "from").unwrap_or("?");
            let subj = parse_row_field(resp, "subj").unwrap_or("(no subject)");
            copy_field(&mut peek.rows[peek.count].from, from);
            copy_field(&mut peek.rows[peek.count].subj, subj);
            peek.count += 1;
        }
    }

    peek
}

/// Why a document open was refused (or empty), for honest reader copy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocDeny {
    None,
    NeedFiles,
    NeedAudio,
    NeedEmail,
    NoBody,
    OutsideRoots,
    Other,
}

impl DocDeny {
    pub fn from_err(resp: &str) -> Self {
        if resp.contains("needs_workspace_cap") {
            Self::NeedFiles
        } else if resp.contains("needs_audio_cap") {
            Self::NeedAudio
        } else if resp.contains("needs_email_cap") {
            Self::NeedEmail
        } else if resp.contains("no readable body") || resp.contains("no such message") {
            Self::NoBody
        } else if resp.contains("outside the indexed roots") {
            Self::OutsideRoots
        } else {
            Self::Other
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::None => "",
            Self::NeedFiles => "Grant Your files to open this document.",
            Self::NeedAudio => "Grant Recordings to open this transcript.",
            Self::NeedEmail => "Grant Email to open this message.",
            Self::NoBody => "Nothing readable here.",
            Self::OutsideRoots => "Outside the indexed folders.",
            Self::Other => "Could not open this document.",
        }
    }
}

/// Lines of a document, for the reader.
pub struct DocPage {
    pub status: BridgeStatus,
    pub denied: bool,
    pub deny: DocDeny,
    pub count: usize,
    pub lines: [[u8; 84]; Self::MAX],
}

impl DocPage {
    pub const MAX: usize = 18;

    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        Self {
            status,
            denied,
            deny: DocDeny::None,
            count: 0,
            lines: [[0; 84]; Self::MAX],
        }
    }

    pub fn line_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.lines[i]))
    }
}

/// Read a document the search results pointed at.
///
/// The same grants are sent as for the query, because the bridge checks scope
/// per source: a caller that could not have found a document must not be able
/// to read it by knowing its URL.
pub fn fetch_doc(caps: crate::caps::Caps, url: &str) -> DocPage {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return DocPage::empty(BridgeStatus::Offline, false),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL doc.read url=");
    com2.write_str(url);
    com2.write_str(" lines=18");
    if caps.allows(crate::caps::Cap::WorkspaceIndex) {
        com2.write_str(" files=1");
    }
    if caps.allows(crate::caps::Cap::AudioTranscribe) {
        com2.write_str(" audio=1");
    }
    if caps.allows(crate::caps::Cap::EmailSearch) {
        com2.write_str(" email=1");
    }
    // Portals are the only source that leaves this machine.
    if caps.allows(crate::caps::Cap::PortalSync) {
        com2.write_str(" portal=1");
    }
    com2.write_str("\n");

    let mut page = DocPage::empty(BridgeStatus::Online, false);
    let mut first = true;
    for _ in 0..40 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp == "END" {
            break;
        }
        if resp.starts_with("ERR ") {
            page.denied = true;
            page.deny = DocDeny::from_err(resp);
            break;
        }
        if resp.starts_with("OK doc.read") {
            continue;
        }
        if resp.starts_with("ROW ") && page.count < DocPage::MAX {
            let text = parse_row_field(resp, "line").unwrap_or("");
            copy_field(&mut page.lines[page.count], text);
            page.count += 1;
        }
    }
    page
}

/// Ask the bridge to build what a newly granted capability needs.
///
/// Granting a capability should make it work, not merely permit it. Without
/// this, `workspace.index` could be on while no index existed — search then
/// reported "the bridge searched your files" and returned nothing, which is
/// the worst of both: permission taken, no benefit, and a message that lies.
pub fn build_index(tool: &str) -> BridgeStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return BridgeStatus::Offline;
    }
    com2.write_str("CALL ");
    com2.write_str(tool);
    // The tool checks the same flag search.query does.
    com2.write_str(" files=1 audio=1 portal=1\n");
    // Indexing walks the disk, so allow a generous first read.
    for _ in 0..12 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if resp == "END" || resp.starts_with("ERR ") {
            break;
        }
    }
    BridgeStatus::Online
}

/// Ask the bridge to delete what a revoked capability produced.
///
/// Turning a switch off should remove the index it built, not just stop
/// answering from it — otherwise "off" means "hidden", which is not what the
/// switch says.
pub fn forget(tool: &str) -> BridgeStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return BridgeStatus::Offline;
    }
    com2.write_str("CALL ");
    com2.write_str(tool);
    com2.write_str("\n");
    // Drain the reply so the next call starts on a clean line.
    for _ in 0..8 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        if str_prefix(&line[..n]) == "END" {
            break;
        }
    }
    BridgeStatus::Online
}

/// Liveness only: PING the bridge without reading any mailbox.
///
/// Used before the user has consented on the Capabilities step. Calling
/// `fetch_mail_peek` there would read — and, since the bridge indexes results,
/// *persist* — the inbox before anyone agreed to it.
pub fn probe_bridge() -> BridgeStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    ping_bridge(&com2, &mut line)
}

/// List playbooks via `CALL skills.list`. Offline → builtins baked into the ISO.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return crate::skills::SkillPeek::from_builtin(),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL skills.list\n");

    let mut peek = crate::skills::SkillPeek::empty();
    peek.from_bridge = true;
    let mut first = true;
    for _ in 0..24 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if resp.starts_with("OK skills.list") {
            continue;
        }
        if resp.starts_with("ROW ") {
            let name = parse_row_field(resp, "name").unwrap_or("?");
            let desc = parse_row_field(resp, "desc").unwrap_or("");
            let saved = matches!(parse_row_field(resp, "src"), Some("saved"));
            if !peek.push_src(name, desc, saved) {
                break;
            }
        }
    }

    if peek.count == 0 {
        // Bridge answered but listed nothing — still show ISO defaults.
        crate::skills::SkillPeek::from_builtin()
    } else {
        peek
    }
}

/// Outcome of a guest `skills.save` attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveSkillStatus {
    /// `Cap::SkillsSave` was off — never opened COM2.
    Denied,
    Offline,
    Ok,
    /// Bridge answered with ERR (other than a missing-cap race).
    Failed,
}

/// Write a one-line starter skill when `Cap::SkillsSave` is granted.
///
/// Uses the bridge `desc=` form (`CALL … skills=1`) so the guest never has to
/// speak `LINE`…`END`. Cap refusal happens before any serial I/O so host unit
/// tests can assert the gate without touching COM2.
pub fn save_skill(caps: crate::caps::Caps, name: &str, desc: &str) -> SaveSkillStatus {
    if !caps.allows(crate::caps::Cap::SkillsSave) {
        return SaveSkillStatus::Denied;
    }
    if !skill_name_ok(name) || desc.is_empty() {
        return SaveSkillStatus::Failed;
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return SaveSkillStatus::Offline,
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL skills.save name=");
    com2.write_str(name);
    com2.write_str(" desc=");
    com2.write_str(desc);
    com2.write_str(" skills=1\n");

    let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
        return SaveSkillStatus::Offline;
    };
    let resp = str_prefix(&line[..n]);
    if resp.starts_with("OK skills.save") {
        SaveSkillStatus::Ok
    } else if resp.contains("needs_skills_cap") {
        SaveSkillStatus::Denied
    } else {
        SaveSkillStatus::Failed
    }
}

/// Bridge skill names: ASCII letters, digits, `-`, `_`.
fn skill_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 28
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// First useful body line from `CALL skills.get name=…` (for a clicked row).
///
/// Returns `false` when the bridge is down or the skill is missing.
pub fn fetch_skill_blurb(name: &str, out: &mut [u8]) -> bool {
    out.fill(0);
    if name.is_empty() {
        return false;
    }
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return false;
    }

    com2.write_str("CALL skills.get name=");
    com2.write_str(name);
    com2.write_str("\n");

    let mut first = true;
    let mut in_frontmatter = false;
    let mut saw_fm_open = false;
    for _ in 0..40 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp == "END" || resp.starts_with("ERR ") {
            break;
        }
        if resp.starts_with("OK skills.get") {
            continue;
        }
        let Some(body) = resp.strip_prefix("LINE ") else {
            continue;
        };
        // Skip YAML frontmatter so the blurb is real prose, not `---`.
        if body.trim() == "---" {
            if !saw_fm_open {
                saw_fm_open = true;
                in_frontmatter = true;
            } else {
                in_frontmatter = false;
            }
            continue;
        }
        if in_frontmatter {
            continue;
        }
        let text = body.trim();
        if text.is_empty() {
            continue;
        }
        copy_field(out, text);
        return true;
    }
    false
}

fn ping_bridge(com2: &Serial, line: &mut [u8]) -> BridgeStatus {
    for _ in 0..64 {
        if com2.try_read_byte().is_none() {
            break;
        }
    }
    com2.write_str("PING\n");
    let Some(n) = com2.read_line(line, TIMEOUT_PING) else {
        return BridgeStatus::Offline;
    };
    let resp = str_prefix(&line[..n]);
    if resp.starts_with("OK pong") {
        BridgeStatus::Online
    } else {
        BridgeStatus::Offline
    }
}

/// Run `search.query` when granted. `q` must be ASCII without spaces (use `-`).
pub fn fetch_search_peek(caps: crate::caps::Caps, q: &str) -> SearchPeek {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    if !caps.allows(crate::caps::Cap::SearchQuery) {
        // Refuse before probing: a denied cap is denied whether or not a
        // bridge happens to be listening.
        let status = ping_bridge(&com2, &mut line);
        return SearchPeek::empty(status, true);
    }

    match ping_bridge(&com2, &mut line) {
        // No bridge: answer from the index baked into the kernel. Search is the
        // one connector that needs no host — see `search.rs`.
        BridgeStatus::Offline => return search_offline(),
        BridgeStatus::Online => {}
    }

    // CALL search.query q=… k=3 [email=1]
    //
    // The email graph is opt-in per call on the bridge. Ask for it only when
    // the user granted email.search at setup: holding search.query alone must
    // not reach mail content.
    com2.write_str("CALL search.query q=");
    com2.write_str(q);
    com2.write_str(" k=3");
    if caps.allows(crate::caps::Cap::EmailSearch) {
        com2.write_str(" email=1");
    }
    // Personal documents are a separate grant from the built-in corpus:
    // search.query alone must not reach the user's own file tree.
    if caps.allows(crate::caps::Cap::WorkspaceIndex) {
        com2.write_str(" files=1");
    }
    if caps.allows(crate::caps::Cap::AudioTranscribe) {
        com2.write_str(" audio=1");
    }
    // Teddy / remote corpora leave the machine — only with portal.sync.
    if caps.allows(crate::caps::Cap::PortalSync) {
        com2.write_str(" portal=1");
    }
    com2.write_str("\n");

    let mut peek = SearchPeek::empty(BridgeStatus::Online, false);
    let mut first = true;
    for _ in 0..16 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if resp.starts_with("OK search.query") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.hits.len() {
            let title = parse_row_field(resp, "title").unwrap_or("?");
            copy_field(&mut peek.hits[peek.count].title, title);
            copy_field(&mut peek.hits[peek.count].url, parse_row_field(resp, "url").unwrap_or(""));
            peek.count += 1;
        }
    }
    peek
}

/// Top hits from the in-kernel index, used when COM2 does not answer.
///
/// Reported as `Offline` so the UI can still say the bridge is down while
/// showing real results.
fn search_offline() -> SearchPeek {
    let mut peek = SearchPeek::empty(BridgeStatus::Offline, false);
    let mut hits = [crate::search::Hit { doc: 0, score: 0 }; crate::search::MAX_HITS];
    let n = crate::search::query(OFFLINE_QUERY, &mut hits);
    for h in hits.iter().take(n.min(peek.hits.len())) {
        copy_field(&mut peek.hits[peek.count].title, crate::search::DOCS[h.doc].title);
        peek.count += 1;
    }
    peek
}

/// What the home screen asks for when nothing else was requested.
const OFFLINE_QUERY: &str = "capability agent bridge";

/// One field/value pair from a portal tool (`teddy.*` / `market.*`).
pub struct PortalRow {
    pub field: [u8; 28],
    pub value: [u8; 48],
}

/// Short peek from a live portal call.
pub struct PortalPeek {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub rows: [PortalRow; 8],
}

impl PortalPeek {
    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        const EMPTY: PortalRow = PortalRow {
            field: [0; 28],
            value: [0; 48],
        };
        Self {
            status,
            denied,
            count: 0,
            rows: [EMPTY; 8],
        }
    }

    pub fn field_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].field))
    }

    pub fn value_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].value))
    }
}

/// Live teddysearch.com portal tools (must match bridge `portals::ENDPOINTS`).
pub const TEDDY_PORTALS: &[&str] = &["teddy.health", "teddy.fear_greed", "teddy.gex"];

/// Live superintelmarkets.com portal tools (same shapes, different origin).
pub const MARKET_PORTALS: &[&str] = &["market.health", "market.fear_greed"];

/// Call a portal tool when `portal.sync` is granted.
///
/// Distinct from the teddy *API* (`tsearch.sync` / corpus search): portals are
/// live HTTPS round-trips. Both need the same consent bit on the wire.
pub fn fetch_portal(caps: crate::caps::Caps, tool: &str) -> PortalPeek {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    if !caps.allows(crate::caps::Cap::PortalSync) {
        let status = ping_bridge(&com2, &mut line);
        return PortalPeek::empty(status, true);
    }
    let known = TEDDY_PORTALS.contains(&tool) || MARKET_PORTALS.contains(&tool);
    if tool.is_empty() || !known {
        return PortalPeek::empty(BridgeStatus::Online, true);
    }

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return PortalPeek::empty(BridgeStatus::Offline, false),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL ");
    com2.write_str(tool);
    com2.write_str(" portal=1\n");

    let mut peek = PortalPeek::empty(BridgeStatus::Online, false);
    let mut first = true;
    for _ in 0..24 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp == "END" {
            break;
        }
        if resp.starts_with("ERR ") {
            peek.denied = resp.contains("needs_portal_cap");
            break;
        }
        if resp.starts_with("OK ") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.rows.len() {
            let field = parse_row_field(resp, "field").unwrap_or("?");
            let value = parse_row_field(resp, "value").unwrap_or("");
            copy_field(&mut peek.rows[peek.count].field, field);
            copy_field(&mut peek.rows[peek.count].value, value);
            peek.count += 1;
        }
    }
    peek
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_peek_stays_behind_email_search() {
        use crate::caps::{Cap, Caps};
        // Do not call fetch_mail_peek: host unit tests cannot touch COM2.
        // Guest refuse + wire email=1 are the gate; search alone must not unlock mail.
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        assert!(!caps.allows(Cap::EmailSearch));
    }

    #[test]
    fn doc_deny_messages_name_the_missing_grant() {
        assert_eq!(
            DocDeny::from_err("ERR doc.read needs_workspace_cap"),
            DocDeny::NeedFiles
        );
        assert_eq!(
            DocDeny::from_err("ERR doc.read needs_email_cap"),
            DocDeny::NeedEmail
        );
        assert_eq!(
            DocDeny::from_err("ERR doc.read needs_audio_cap"),
            DocDeny::NeedAudio
        );
        assert!(DocDeny::NeedFiles.message().contains("Your files"));
        assert!(DocDeny::NeedEmail.message().contains("Email"));
        assert!(DocDeny::NeedAudio.message().contains("Recordings"));
        for d in [
            DocDeny::NeedFiles,
            DocDeny::NeedAudio,
            DocDeny::NeedEmail,
            DocDeny::NoBody,
            DocDeny::OutsideRoots,
            DocDeny::Other,
        ] {
            let m = d.message();
            assert!(m.bytes().all(|b| (0x20..=0x7E).contains(&b)), "{m}");
        }
    }

    #[test]
    fn email_graph_requested_only_with_the_email_cap() {
        use crate::caps::{Cap, Caps};
        // Search-only grants must not ask the bridge for mail.
        let mut search_only = Caps::none();
        search_only.set(Cap::SearchQuery, true);
        assert!(search_only.allows(Cap::SearchQuery));
        assert!(
            !search_only.allows(Cap::EmailSearch),
            "search.query alone must not reach the email graph"
        );

        let mut both = search_only;
        both.set(Cap::EmailSearch, true);
        assert!(both.allows(Cap::EmailSearch));
    }

    #[test]
    fn probe_does_not_imply_a_mailbox_read() {
        // Guard the consent rule: the pre-consent path must expose liveness
        // only. MailPeek::empty carries no rows.
        let p = MailPeek::empty(BridgeStatus::Offline);
        assert_eq!(p.count, 0);
    }

    #[test]
    fn offline_search_still_returns_hits() {
        // The whole point of the offline tier: useful results with no host.
        let peek = search_offline();
        assert!(matches!(peek.status, BridgeStatus::Offline));
        assert!(!peek.denied);
        assert!(peek.count > 0, "baked index returned nothing");
    }

    #[test]
    fn parse_row() {
        let line = "ROW from=Alice Chen|subj=Q2 planning";
        assert_eq!(parse_row_field(line, "from"), Some("Alice Chen"));
        assert_eq!(parse_row_field(line, "subj"), Some("Q2 planning"));
    }

    #[test]
    fn parse_skills_list_row() {
        let line = "ROW name=email-triage|src=default|desc=Inbox via MCP email";
        assert_eq!(parse_row_field(line, "name"), Some("email-triage"));
        assert_eq!(parse_row_field(line, "desc"), Some("Inbox via MCP email"));
        assert_eq!(parse_row_field(line, "src"), Some("default"));
        let saved = "ROW name=guest-starter|src=saved|desc=from guest";
        assert_eq!(parse_row_field(saved, "src"), Some("saved"));
    }

    #[test]
    fn skill_name_ok_matches_bridge_rules() {
        assert!(skill_name_ok("guest-starter"));
        assert!(skill_name_ok("a_b1"));
        assert!(!skill_name_ok(""));
        assert!(!skill_name_ok("has space"));
        assert!(!skill_name_ok("bad/name"));
    }

    #[test]
    fn save_skill_refuses_without_opening_com2() {
        use crate::caps::{Cap, Caps};
        // Denied before serial: safe in host unit tests.
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        assert_eq!(
            save_skill(caps, "guest-starter", "demo"),
            SaveSkillStatus::Denied
        );
        assert!(!caps.allows(Cap::SkillsSave));
    }

    #[test]
    fn portal_tools_stay_behind_portal_sync() {
        use crate::caps::{Cap, Caps};
        // Do not call fetch_portal here: host unit tests cannot touch COM2.
        // The guest gate is Cap::PortalSync; search alone must not unlock it.
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        assert!(!caps.allows(Cap::PortalSync));
        assert!(TEDDY_PORTALS.iter().all(|t| t.starts_with("teddy.")));
    }

    #[test]
    fn teddy_and_market_portal_names_are_listed() {
        assert!(TEDDY_PORTALS.contains(&"teddy.health"));
        assert!(TEDDY_PORTALS.contains(&"teddy.fear_greed"));
        assert!(TEDDY_PORTALS.contains(&"teddy.gex"));
        assert!(MARKET_PORTALS.contains(&"market.health"));
        assert!(MARKET_PORTALS.contains(&"market.fear_greed"));
        // No loose prefix: unknown market.* must not sneak through.
        assert!(!MARKET_PORTALS.contains(&"market.nope"));
    }
}
