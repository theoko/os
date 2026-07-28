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

/// True when this image was built for bare metal, where no host will ever be
/// attached to COM2.
///
/// Deliberately `cfg!` inside a function rather than `#[cfg]` around each call
/// site: both arms keep type-checking in either configuration, so the bridge
/// paths cannot rot while the standalone image is the one being built.
pub const fn standalone() -> bool {
    cfg!(feature = "standalone")
}

pub struct MailRow {
    /// Graph id from the bridge (`id=`), used to open `email://…`.
    pub id: [u8; 20],
    /// Source URL, so a listed message can be opened rather than only shown.
    /// Present when the bridge sends one; `id` is the fallback route.
    pub url: [u8; 72],
    pub from: [u8; 40],
    pub subj: [u8; 72],
}

pub struct MailPeek {
    pub status: BridgeStatus,
    /// The host deliberately has no signed-in mailbox; this is distinct from
    /// an empty inbox and lets the UI offer the right next step.
    pub needs_connection: bool,
    pub count: usize,
    pub rows: [MailRow; 5],
}

impl MailPeek {
    pub const fn empty(status: BridgeStatus) -> Self {
        const EMPTY: MailRow = MailRow {
            id: [0; 20],
            url: [0; 72],
            from: [0; 40],
            subj: [0; 72],
        };
        Self {
            status,
            needs_connection: false,
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

    pub fn row_id(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].id))
    }

    /// Source URL the bridge sent for this row, when it sent one.
    pub fn row_url(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].url))
    }

    /// Build `email://{id}` into `buf`. Empty when the row has no id.
    pub fn url_at<'a>(&self, i: usize, buf: &'a mut [u8; 40]) -> Option<&'a str> {
        let id = self.row_id(i);
        if id.is_empty() || id.len() > 24 {
            return None;
        }
        buf.fill(0);
        let prefix = b"email://";
        let n = prefix.len() + id.len();
        if n > buf.len() {
            return None;
        }
        buf[..prefix.len()].copy_from_slice(prefix);
        buf[prefix.len()..n].copy_from_slice(id.as_bytes());
        Some(str_prefix(&buf[..n]))
    }
}

/// One row from `workspace.recent` (needs `files=1` / Your files).
pub struct FileRow {
    pub title: [u8; 48],
    pub url: [u8; 72],
}

/// Short workspace peek for the home Recent files strip.
pub struct FilePeek {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub rows: [FileRow; 3],
}

impl FilePeek {
    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        const EMPTY: FileRow = FileRow {
            title: [0; 48],
            url: [0; 72],
        };
        Self {
            status,
            denied,
            count: 0,
            rows: [EMPTY; 3],
        }
    }

    pub fn title_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].title))
    }

    pub fn url_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].url))
    }
}

/// One row from `calendar.list` (same `email=1` consent as mail).
pub struct CalRow {
    pub id: [u8; 20],
    pub title: [u8; 48],
    pub when: [u8; 32],
}

/// Short calendar peek for morning / playbook act.
pub struct CalendarPeek {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub rows: [CalRow; 3],
}

impl CalendarPeek {
    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        const EMPTY: CalRow = CalRow {
            id: [0; 20],
            title: [0; 48],
            when: [0; 32],
        };
        Self {
            status,
            denied,
            count: 0,
            rows: [EMPTY; 3],
        }
    }

    pub fn id_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].id))
    }

    pub fn title_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].title))
    }

    pub fn when_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.rows[i].when))
    }

    /// Build `cal://{id}` into `buf`. Empty when the row has no id.
    pub fn url_at<'a>(&self, i: usize, buf: &'a mut [u8; 40]) -> Option<&'a str> {
        let id = self.id_at(i);
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
        Some(str_prefix(&buf[..n]))
    }
}

/// One hit from `search.query`.
pub struct SearchHit {
    pub title: [u8; 48],
    /// Source URL, needed to open the document rather than only name it.
    pub url: [u8; 72],
}

/// Structured plan from host `intent.resolve` (smart Home asks).
pub struct IntentPlan {
    pub status: BridgeStatus,
    pub act: [u8; 12],
    pub query: [u8; 48],
    pub plan_n: usize,
    pub plans: [[u8; 52]; 4],
    pub hit_n: usize,
    pub hits: [SearchHit; 3],
}

impl IntentPlan {
    pub const fn empty(status: BridgeStatus) -> Self {
        const EMPTY: SearchHit = SearchHit {
            title: [0; 48],
            url: [0; 72],
        };
        Self {
            status,
            act: [0; 12],
            query: [0; 48],
            plan_n: 0,
            plans: [[0; 52]; 4],
            hit_n: 0,
            hits: [EMPTY; 3],
        }
    }

    pub fn act_at(&self) -> &str {
        str_prefix(trim_buf(&self.act))
    }

    pub fn query_at(&self) -> &str {
        str_prefix(trim_buf(&self.query))
    }

    pub fn plan_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.plans[i]))
    }

    pub fn hit_title_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.hits[i].title))
    }

    pub fn hit_url_at(&self, i: usize) -> &str {
        str_prefix(trim_buf(&self.hits[i].url))
    }
}

/// How long a sentence the agent may say back. One line at BODY size.
pub const SAY_MAX: usize = 156;

/// Short corpus peek for the home Connectors card.
pub struct SearchPeek {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub hits: [SearchHit; crate::search::MAX_HITS],
    /// What the agent understood, in a sentence. Empty when nothing said it.
    pub say: [u8; SAY_MAX],
}

impl SearchPeek {
    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        const EMPTY: SearchHit = SearchHit {
            title: [0; 48],
            url: [0; 72],
        };
        Self {
            status,
            denied,
            count: 0,
            hits: [EMPTY; crate::search::MAX_HITS],
            say: [0; SAY_MAX],
        }
    }

    /// The agent's sentence, or empty if it did not produce one.
    pub fn say(&self) -> &str {
        str_prefix(trim_buf(&self.say))
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

/// Copy a URL, or store nothing when it does not fit.
///
/// A cut URL is worse than no URL. It still reads as non-empty, so the row
/// is drawn as openable, and the tap resolves to a path the bridge cannot
/// find — the same broken promise as an unarmed `Doc` row. Two documents
/// whose paths share the first 72 bytes also compare equal once cut, so the
/// second is dropped as a duplicate of the first.
///
/// Callers read an empty URL as "found it, cannot open it", which is true.
/// Deep workspace roots reach this: the bridge sends paths up to 90 bytes.
pub fn copy_url(dst: &mut [u8], src: &str) {
    dst.fill(0);
    if src.len() > dst.len() {
        return;
    }
    dst[..src.len()].copy_from_slice(src.as_bytes());
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
    // This one probes inline rather than via ping_bridge; keep it gated too.
    if standalone() {
        return MailPeek::empty(BridgeStatus::Offline);
    }
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
        if resp.starts_with("ERR email.search email_not_connected") {
            peek.needs_connection = true;
            break;
        }
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if resp.starts_with("OK email.search") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.rows.len() {
            let id = parse_row_field(resp, "id").unwrap_or("");
            let from = parse_row_field(resp, "from").unwrap_or("?");
            let subj = parse_row_field(resp, "subj").unwrap_or("(no subject)");
            copy_field(&mut peek.rows[peek.count].id, id);
            copy_field(&mut peek.rows[peek.count].from, from);
            copy_field(&mut peek.rows[peek.count].subj, subj);
            copy_url(
                &mut peek.rows[peek.count].url,
                parse_row_field(resp, "url").unwrap_or(""),
            );
            peek.count += 1;
        }
    }

    peek
}

/// Peek top-ranked workspace files for Home. Cap refusal never opens COM2.
pub fn fetch_files_peek(caps: crate::caps::Caps) -> FilePeek {
    if !caps.allows(crate::caps::Cap::WorkspaceIndex) {
        return FilePeek::empty(BridgeStatus::Offline, true);
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return FilePeek::empty(BridgeStatus::Offline, false),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL workspace.recent k=3 files=1\n");

    let mut peek = FilePeek::empty(BridgeStatus::Online, false);
    let mut first = true;
    for _ in 0..16 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            if resp.contains("needs_workspace_cap") {
                peek.denied = true;
            }
            break;
        }
        if resp.starts_with("OK workspace.recent") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.rows.len() {
            let title = parse_row_field(resp, "title").unwrap_or("(file)");
            let url = parse_row_field(resp, "url").unwrap_or("");
            if !url.starts_with("file://") {
                continue;
            }
            copy_field(&mut peek.rows[peek.count].title, title);
            copy_url(&mut peek.rows[peek.count].url, url);
            peek.count += 1;
        }
    }
    peek
}

/// Peek upcoming calendar events. Same consent as mail (`Cap::EmailSearch`).
///
/// Cap refusal never opens COM2 — calendar is not ambient just because LIST
/// names the tool.
pub fn fetch_calendar_peek(caps: crate::caps::Caps) -> CalendarPeek {
    if !caps.allows(crate::caps::Cap::EmailSearch) {
        return CalendarPeek::empty(BridgeStatus::Offline, true);
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return CalendarPeek::empty(BridgeStatus::Offline, false),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL calendar.list email=1\n");

    let mut peek = CalendarPeek::empty(BridgeStatus::Online, false);
    let mut first = true;
    for _ in 0..16 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            if resp.contains("needs_email_cap") {
                peek.denied = true;
            }
            break;
        }
        if resp.starts_with("OK calendar.list") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.rows.len() {
            let id = parse_row_field(resp, "id").unwrap_or("");
            let title = parse_row_field(resp, "title").unwrap_or("(event)");
            let when = parse_row_field(resp, "when").unwrap_or("");
            copy_field(&mut peek.rows[peek.count].id, id);
            copy_field(&mut peek.rows[peek.count].title, title);
            copy_field(&mut peek.rows[peek.count].when, when);
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
    NeedPortal,
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
        } else if resp.contains("needs_portal_cap") {
            Self::NeedPortal
        } else if resp.contains("no readable body")
            || resp.contains("no such message")
            || resp.contains("no such transcript")
            || resp.contains("no such event")
        {
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
            Self::NeedEmail => "Grant Email to open mail or calendar.",
            Self::NeedPortal => "Grant Online services to open this page.",
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
    /// Lines held for the reader. Enough that most documents fit entirely
    /// and scrolling is local; the screen shows ~22 at a time.
    pub const MAX: usize = 120;

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
        // No bridge: read the extract baked into the kernel, the same way a
        // query falls back to the baked index. Doing it here rather than in
        // each caller means every route to the reader — a Search row, a Brief
        // Doc row — opens the same document.
        BridgeStatus::Offline => return doc_offline(url),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL doc.read url=");
    com2.write_str(url);
    com2.write_str(" lines=120");
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
    for _ in 0..(DocPage::MAX + 8) {
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

/// What the portal source currently holds, locally.
pub struct PortalStatus {
    pub reachable: bool,
    pub cached: bool,
    pub syncing: bool,
    pub docs: usize,
}

/// Read cached-corpus status. Does not cause a fetch.
pub fn portal_status() -> PortalStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return PortalStatus {
            reachable: false,
            cached: false,
            syncing: false,
            docs: 0,
        };
    }
    com2.write_str("CALL portal.status\n");
    let mut st = PortalStatus {
        reachable: true,
        cached: false,
        syncing: false,
        docs: 0,
    };
    for _ in 0..8 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if resp == "END" || resp.starts_with("ERR ") {
            break;
        }
        if let Some(rest) = resp.strip_prefix("OK portal.status ") {
            for field in rest.split_whitespace() {
                if let Some(v) = field.strip_prefix("n=") {
                    st.docs = v.parse().unwrap_or(0);
                } else if let Some(v) = field.strip_prefix("cached=") {
                    st.cached = v == "1";
                } else if let Some(v) = field.strip_prefix("syncing=") {
                    st.syncing = v == "1";
                }
            }
        }
    }
    st
}

// --- portal config (hidden screen) -----------------------------------------
//
// Three calls behind the Ctrl+Shift+P screen. Nothing here writes to COM1: the
// password must never reach the serial log, so it is never handed to anything
// that logs, and the reply lines carry no secret to leak back.

/// Which portal family the host talks to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortalFamily {
    Teddy,
    Market,
    /// Configured to reach nothing at all.
    None,
}

impl PortalFamily {
    pub const ALL: [PortalFamily; 3] =
        [PortalFamily::Teddy, PortalFamily::Market, PortalFamily::None];

    /// The `family=` token on the wire. Always one of three literals, so it is
    /// always safe to splice into a CALL line.
    pub fn wire(self) -> &'static str {
        match self {
            PortalFamily::Teddy => "teddy",
            PortalFamily::Market => "market",
            PortalFamily::None => "none",
        }
    }

    /// What the person choosing it reads on screen.
    pub fn label(self) -> &'static str {
        match self {
            PortalFamily::Teddy => "teddysearch.com",
            PortalFamily::Market => "superintelmarkets.com",
            PortalFamily::None => "None (offline)",
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        PortalFamily::ALL.into_iter().find(|f| f.wire() == s)
    }
}

/// Reply to `CALL config.status`.
///
/// Unlock is per-connection on the host, so `locked` is session state that can
/// come back at any time. Re-read this rather than remembering an old answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfigStatus {
    pub reachable: bool,
    pub family: PortalFamily,
    pub locked: bool,
    pub configured: bool,
}

impl ConfigStatus {
    /// What to believe when the bridge never answered: locked, nothing set.
    /// Failing closed matters more here than anywhere else on the machine.
    pub const fn offline() -> Self {
        Self {
            reachable: false,
            family: PortalFamily::None,
            locked: true,
            configured: false,
        }
    }
}

/// `CALL config.status` -> `OK config.status portal=… locked=… configured=…`.
pub fn config_status() -> ConfigStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return ConfigStatus::offline();
    }
    com2.write_str("CALL config.status\n");

    let mut st = ConfigStatus {
        reachable: true,
        ..ConfigStatus::offline()
    };
    for _ in 0..8 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if resp == "END" || resp.starts_with("ERR ") {
            break;
        }
        if let Some(rest) = resp.strip_prefix("OK config.status ") {
            for field in rest.split_whitespace() {
                if let Some(v) = field.strip_prefix("portal=") {
                    st.family = PortalFamily::from_wire(v).unwrap_or(PortalFamily::None);
                } else if let Some(v) = field.strip_prefix("locked=") {
                    st.locked = v == "1";
                } else if let Some(v) = field.strip_prefix("configured=") {
                    st.configured = v == "1";
                }
            }
        }
    }
    st
}

/// Longest secret the guest will frame into a CALL line.
pub const PASS_MAX: usize = 32;

/// Can this secret be spliced into a whitespace-delimited CALL line at all?
///
/// The request builders in this module concatenate raw values into one line
/// and the wire has no escape syntax. A space would split the secret into a
/// second argument; a newline would end the request and let the tail arrive as
/// a forged one. Neither can be encoded, so the only safe answer is to refuse
/// to send — the caller says so on screen and COM2 is never opened.
pub fn pass_frameable(pass: &str) -> bool {
    if pass.is_empty() || pass.len() > PASS_MAX {
        return false;
    }
    // A space is safe even though the wire is whitespace-delimited, because
    // `pass=` is the last argument of the request and the host folds trailing
    // tokens back into its value. Allowing it is what makes a memorable
    // passphrase usable instead of forcing one unbroken token.
    //
    // `=` is the byte that actually cannot be allowed: the host starts a NEW
    // argument at any token shaped `key=value`, so "correct horse=x" would
    // silently arrive as "correct" and the unlock would fail for a reason
    // nobody could see. `|` is the field separator elsewhere in the protocol.
    if pass.bytes().any(|b| b == b'=' || b == b'|') {
        return false;
    }
    // Leading and trailing spaces cannot survive the trip — the host trims the
    // line, and a token boundary swallows the rest — so a secret that depends
    // on them would be accepted here and rejected there.
    if pass.starts_with(' ') || pass.ends_with(' ') {
        return false;
    }
    // Printable ASCII only: no control byte (\n, \r, \t) can end the request
    // early and let its tail arrive as a forged one, and nothing outside this
    // range is maskable by the font.
    pass.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

/// Outcome of `CALL config.unlock`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnlockStatus {
    Ok,
    BadPass,
    /// The host has no password set; there is nothing to unlock.
    NotConfigured,
    TooMany,
    Offline,
    /// The secret could not be framed onto the wire. Never sent.
    Unsendable,
}

/// `CALL config.unlock pass=<secret>`.
///
/// The secret is written to COM2 and nowhere else — never to COM1, never into
/// a status buffer, never into a reply this function returns.
pub fn config_unlock(pass: &str) -> UnlockStatus {
    // Checked before any serial I/O: an unframeable secret must not reach the
    // wire even partially.
    if !pass_frameable(pass) {
        return UnlockStatus::Unsendable;
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return UnlockStatus::Offline;
    }

    com2.write_str("CALL config.unlock pass=");
    com2.write_str(pass);
    com2.write_str("\n");

    let mut status = UnlockStatus::Offline;
    for _ in 0..8 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("OK config.unlock") {
            status = UnlockStatus::Ok;
        } else if resp.starts_with("ERR config.unlock") {
            status = if resp.contains("not_configured") {
                UnlockStatus::NotConfigured
            } else if resp.contains("too_many") {
                UnlockStatus::TooMany
            } else {
                UnlockStatus::BadPass
            };
            break;
        }
        if resp == "END" {
            break;
        }
    }
    status
}

/// Outcome of `CALL config.portal`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortalSetStatus {
    /// Accepted; carries the family the host echoed back.
    Ok(PortalFamily),
    /// The session lost its unlock. Ask for the password again.
    Locked,
    UnknownFamily,
    Offline,
    Failed,
}

/// `CALL config.portal family=<teddy|market|none>`.
pub fn config_portal(family: PortalFamily) -> PortalSetStatus {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return PortalSetStatus::Offline;
    }

    com2.write_str("CALL config.portal family=");
    com2.write_str(family.wire());
    com2.write_str("\n");

    let mut status = PortalSetStatus::Offline;
    for _ in 0..8 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if let Some(rest) = resp.strip_prefix("OK config.portal") {
            // Trust the echo over the request: the host is the authority on
            // what it actually stored.
            let echoed = rest
                .split_whitespace()
                .find_map(|f| f.strip_prefix("family="))
                .and_then(PortalFamily::from_wire);
            status = PortalSetStatus::Ok(echoed.unwrap_or(family));
        } else if resp.starts_with("ERR config.portal") {
            status = if resp.contains("locked") {
                PortalSetStatus::Locked
            } else if resp.contains("unknown_family") {
                PortalSetStatus::UnknownFamily
            } else {
                PortalSetStatus::Failed
            };
            break;
        }
        if resp == "END" {
            break;
        }
    }
    status
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

/// Outcome of a guest `email.send` attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SendMailStatus {
    /// `Cap::EmailSend` was off — never opened COM2.
    Denied,
    Offline,
    Ok,
    Failed,
}

/// Send a message when Send mail is granted. Always includes `confirm=1`.
///
/// Cap refusal never opens COM2. The bridge still rejects calls without
/// `confirm=1`, so a forged guest cannot skip the Brief Confirm send step by
/// omitting the wire bit alone — the Cap is what arms `confirm=1`.
pub fn send_mail(
    caps: crate::caps::Caps,
    to: &str,
    subj: &str,
    body: &str,
) -> SendMailStatus {
    if !caps.allows(crate::caps::Cap::EmailSend) {
        return SendMailStatus::Denied;
    }
    if to.is_empty()
        || to.contains('|')
        || to.contains('\n')
        || subj.contains('|')
        || subj.contains('\n')
        || body.contains('|')
        || body.contains('\n')
    {
        return SendMailStatus::Failed;
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];
    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return SendMailStatus::Offline,
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL email.send to=");
    com2.write_str(to);
    com2.write_str(" subj=");
    com2.write_str(subj);
    com2.write_str(" body=");
    com2.write_str(body);
    com2.write_str(" email=1 confirm=1\n");

    let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
        return SendMailStatus::Offline;
    };
    let resp = str_prefix(&line[..n]);
    if resp.starts_with("OK email.send") {
        SendMailStatus::Ok
    } else {
        SendMailStatus::Failed
    }
}

/// Outcome of a guest `audio.transcribe` attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TranscribeStatus {
    /// `Cap::AudioTranscribe` was off — never opened COM2.
    Denied,
    Offline,
    Ok,
    /// Bridge answered with ERR (missing file, not media, whisper, …).
    Failed,
}

/// Ask the bridge to transcribe a host media path when Recordings is on.
///
/// Cap refusal happens before any serial I/O so host unit tests stay safe.
/// The path must already look like media (`searchui::is_media_path`); the
/// bridge still re-checks extension and existence.
pub fn transcribe(caps: crate::caps::Caps, path: &str) -> TranscribeStatus {
    if !caps.allows(crate::caps::Cap::AudioTranscribe) {
        return TranscribeStatus::Denied;
    }
    if path.is_empty() || path.contains(' ') || path.contains('|') || path.contains('\n') {
        return TranscribeStatus::Failed;
    }

    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return TranscribeStatus::Offline,
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL audio.transcribe path=");
    com2.write_str(path);
    com2.write_str(" audio=1\n");

    let mut status = TranscribeStatus::Offline;
    for _ in 0..16 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_REPLY) else {
            break;
        };
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("OK audio.transcribe") {
            status = TranscribeStatus::Ok;
        } else if resp.contains("needs_audio_cap") {
            status = TranscribeStatus::Denied;
            break;
        } else if resp.starts_with("ERR ") {
            status = TranscribeStatus::Failed;
            break;
        }
        if resp == "END" {
            break;
        }
    }
    status
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
    fetch_skill_body(name, out) > 0
}

/// Playbook body from `CALL skills.get`, frontmatter stripped, into `out`.
///
/// Concatenates body lines with newlines so [`crate::agent::enrich_playbook`]
/// can scan for tool names. Returns bytes written (0 = offline / missing).
pub fn fetch_skill_body(name: &str, out: &mut [u8]) -> usize {
    out.fill(0);
    if name.is_empty() || out.is_empty() {
        return 0;
    }
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    if matches!(ping_bridge(&com2, &mut line), BridgeStatus::Offline) {
        return 0;
    }

    com2.write_str("CALL skills.get name=");
    com2.write_str(name);
    com2.write_str("\n");

    let mut first = true;
    let mut in_frontmatter = false;
    let mut saw_fm_open = false;
    let mut wrote = 0usize;
    for _ in 0..(DocPage::MAX + 8) {
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
        // Skip YAML frontmatter so the body is real prose, not `---`.
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
        if wrote > 0 && wrote < out.len() {
            out[wrote] = b'\n';
            wrote += 1;
        }
        let raw = body.as_bytes();
        let room = out.len().saturating_sub(wrote);
        let n = raw.len().min(room);
        out[wrote..wrote + n].copy_from_slice(&raw[..n]);
        wrote += n;
        if wrote >= out.len() {
            break;
        }
    }
    wrote
}

fn ping_bridge(com2: &Serial, line: &mut [u8]) -> BridgeStatus {
    // Standalone hardware has nothing on the far side of COM2, so asking costs
    // TIMEOUT_PING and always answers Offline. Every caller already handles
    // Offline; this only spares them the wait.
    if standalone() {
        return BridgeStatus::Offline;
    }
    if !com2.available() {
        return BridgeStatus::Offline;
    }
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

/// Ask the host to plan a natural-language Home goal.
///
/// Always allowed to CALL (planning is not a personal-data read). File hits
/// on the wire still require `files=1` so the bridge only ranks the index when
/// Your files is granted.
pub fn fetch_intent_plan(caps: crate::caps::Caps, goal: &str) -> IntentPlan {
    let com2 = Serial::com2();
    com2.init();
    let mut line = [0u8; LINE_BUF];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return IntentPlan::empty(BridgeStatus::Offline),
        BridgeStatus::Online => {}
    }

    com2.write_str("CALL intent.resolve q=");
    com2.write_str(goal);
    if caps.allows(crate::caps::Cap::WorkspaceIndex) {
        com2.write_str(" files=1");
    }
    if caps.allows(crate::caps::Cap::EmailSearch) {
        com2.write_str(" email=1");
    }
    com2.write_str("\n");

    let mut plan = IntentPlan::empty(BridgeStatus::Online);
    let mut first = true;
    for _ in 0..20 {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(&mut line, timeout) else {
            break;
        };
        first = false;
        let resp = str_prefix(&line[..n]);
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if let Some(rest) = resp.strip_prefix("OK intent.resolve ") {
            for field in rest.split_whitespace() {
                if let Some(v) = field.strip_prefix("act=") {
                    copy_field(&mut plan.act, v);
                } else if let Some(v) = field.strip_prefix("query=") {
                    // query may continue with spaces — take the remainder once.
                    let q = rest
                        .split_once("query=")
                        .map(|(_, q)| q)
                        .unwrap_or(v);
                    copy_field(&mut plan.query, q);
                    break;
                }
            }
            continue;
        }
        if let Some(p) = resp.strip_prefix("ROW plan=") {
            if plan.plan_n < plan.plans.len() {
                copy_field(&mut plan.plans[plan.plan_n], p);
                plan.plan_n += 1;
            }
            continue;
        }
        if resp.starts_with("ROW ") && plan.hit_n < plan.hits.len() {
            let title = parse_row_field(resp, "title").unwrap_or("(doc)");
            let url = parse_row_field(resp, "url").unwrap_or("");
            if url.is_empty() {
                continue;
            }
            copy_field(&mut plan.hits[plan.hit_n].title, title);
            copy_url(&mut plan.hits[plan.hit_n].url, url);
            plan.hit_n += 1;
        }
    }
    plan
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
        BridgeStatus::Offline => return search_offline(q),
        BridgeStatus::Online => {}
    }

    // CALL agent.act goal=… [email=1] …
    //
    // Not search.query: what people type is a sentence ("i wanna work on my
    // paper"), and a keyword index throws away exactly the words that carry
    // the intent. The agent reads the goal, then falls back to the same scoped
    // index when the goal is really just keywords — so this is never worse.
    //
    // Every source stays opt-in per call. The guest sets a flag only for a
    // capability granted at setup: holding search alone must not reach mail.
    // Ask for exactly what this screen can render. It was hardcoded to 3
    // while `SearchPeek` grew to hold `search::MAX_HITS`, so two rows of
    // every answer were left on the table.
    com2.write_str("CALL agent.act max=");
    com2.write_str(max_rows_str());
    com2.write_str(" goal=");
    com2.write_str(q);
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
    // Teddy / market / portal-backed results are the one source that can leave
    // the machine — only with portal.sync. Carry the explicit grant through to
    // the agent; without this, turning on Online services changed the UI but
    // the agent still searched as if it were denied.
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
        if resp.starts_with("OK ") {
            continue;
        }
        // The one sentence explaining what it understood and what it found.
        if let Some(said) = resp.strip_prefix("SAY ") {
            copy_field(&mut peek.say, said);
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.hits.len() {
            let title = parse_row_field(resp, "title").unwrap_or("?");
            copy_field(&mut peek.hits[peek.count].title, title);
            copy_url(
                &mut peek.hits[peek.count].url,
                parse_row_field(resp, "url").unwrap_or(""),
            );
            peek.count += 1;
        }
    }
    peek
}

/// `search::MAX_HITS` as a string, without a formatter.
///
/// The guest and the bridge have to agree on how many rows an answer holds:
/// the agent's sentence counts them, so a mismatch makes it say "5 matches"
/// above three rows.
fn max_rows_str() -> &'static str {
    match crate::search::MAX_HITS {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        _ => "8",
    }
}

/// Top hits from the in-kernel index, used when COM2 does not answer.
///
/// Reported as `Offline` so the UI can still say the bridge is down while
/// showing real results. The caller's own query is what gets ranked — an
/// earlier version searched a hardcoded showcase phrase instead, so a brief
/// on "nvda" reported corpus docs that had nothing to do with nvda while
/// claiming "local keywords only".
///
/// Titles only, deliberately: document bodies live on the bridge (the kernel
/// bakes an index, not content — see `search.rs`), so nothing found offline
/// can actually open. Leaving the URL empty is what makes the Brief render
/// these as `Hit` rows instead of promising an openable `Doc`.
fn search_offline(q: &str) -> SearchPeek {
    let mut peek = SearchPeek::empty(BridgeStatus::Offline, false);
    let mut hits = [crate::search::Hit { doc: 0, score: 0 }; crate::search::MAX_HITS];
    let q = if q.is_empty() { OFFLINE_QUERY } else { q };
    let n = crate::search::query(q, &mut hits);
    for h in hits.iter().take(n.min(peek.hits.len())) {
        let doc = &crate::search::DOCS[h.doc];
        copy_field(&mut peek.hits[peek.count].title, doc.title);
        // The URL is the promise that the row opens, so it is carried only
        // when this image stores something to open. A document baked without
        // an extract stays a finding — `Brief::push_result` reads the same
        // absence and tags it `Hit`.
        if crate::search::body_for(doc.url).is_some() {
            copy_url(&mut peek.hits[peek.count].url, doc.url);
        }
        peek.count += 1;
    }
    peek
}

/// Fill a reader page from the extract this image stores for `url`.
///
/// A page with no lines is not a broken connection here: it means the image
/// holds no text for that document, which is what the reader then says.
fn doc_offline(url: &str) -> DocPage {
    let mut page = DocPage::empty(BridgeStatus::Offline, false);
    let Some(body) = crate::search::body_for(url) else {
        return page;
    };
    wrap_into(&mut page, body);
    // Without this the stored opening reads as the whole document. Say where
    // the text stops — once, at the end, after a blank line.
    if page.count + 2 <= DocPage::MAX {
        page.count += 1;
        copy_field(&mut page.lines[page.count], crate::copy::extract_only());
        page.count += 1;
    }
    page
}

/// Wrap prose into the reader's fixed line slots, breaking at spaces.
///
/// A word wider than a line is broken rather than dropped: these slots cut in
/// silence, and half a path is a different path.
fn wrap_into(page: &mut DocPage, text: &str) {
    const WRAP: usize = 76;
    let mut line = [0u8; 84];
    let mut w = 0usize;
    let flush = |line: &mut [u8; 84], w: &mut usize, page: &mut DocPage| {
        if *w > 0 && page.count < DocPage::MAX {
            copy_field(&mut page.lines[page.count], str_prefix(&line[..*w]));
            page.count += 1;
        }
        line.fill(0);
        *w = 0;
    };
    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }
        if w > 0 && w + 1 + word.len() > WRAP {
            flush(&mut line, &mut w, page);
        }
        if w > 0 {
            line[w] = b' ';
            w += 1;
        }
        for &b in word.as_bytes() {
            if w == WRAP {
                flush(&mut line, &mut w, page);
            }
            line[w] = b;
            w += 1;
        }
        if page.count >= DocPage::MAX {
            return;
        }
    }
    flush(&mut line, &mut w, page);
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
    fn calendar_peek_refuses_without_opening_com2() {
        use crate::caps::Caps;
        // Cap denial must not touch the serial — same rule as transcribe/save.
        let peek = fetch_calendar_peek(Caps::none());
        assert!(peek.denied);
        assert_eq!(peek.count, 0);
    }

    #[test]
    fn files_peek_refuses_without_opening_com2() {
        use crate::caps::Caps;
        let peek = fetch_files_peek(Caps::none());
        assert!(peek.denied);
        assert_eq!(peek.count, 0);
    }

    #[test]
    fn send_mail_refuses_without_opening_com2() {
        use crate::caps::{Cap, Caps};
        assert_eq!(
            send_mail(Caps::none(), "ada@x.com", "Hi", "Hello"),
            SendMailStatus::Denied
        );
        // Email read alone must not arm send.
        let mut caps = Caps::none();
        caps.set(Cap::EmailSearch, true);
        assert_eq!(
            send_mail(caps, "ada@x.com", "Hi", "Hello"),
            SendMailStatus::Denied
        );
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
        assert_eq!(
            DocDeny::from_err("ERR doc.read no such transcript"),
            DocDeny::NoBody
        );
        assert_eq!(
            DocDeny::from_err("ERR doc.read needs_portal_cap"),
            DocDeny::NeedPortal
        );
        for d in [
            DocDeny::NeedFiles,
            DocDeny::NeedAudio,
            DocDeny::NeedEmail,
            DocDeny::NeedPortal,
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
        // An empty ask falls back to the showcase query.
        let peek = search_offline("");
        assert!(matches!(peek.status, BridgeStatus::Offline));
        assert!(!peek.denied);
        assert!(peek.count > 0, "baked index returned nothing");
    }

    #[test]
    fn offline_search_ranks_the_askers_query_not_a_showcase() {
        // A query the corpus knows nothing about must come back empty.
        // The old code searched a hardcoded phrase instead, so a brief on
        // "nvda" reported unrelated docs under "local keywords only".
        let peek = search_offline("nvda");
        assert_eq!(peek.count, 0, "corpus has no nvda doc, yet hits came back");

        // A query the corpus does know answers with matching titles.
        let peek = search_offline("skills");
        assert!(peek.count > 0, "corpus should answer for its own topics");
    }

    #[test]
    fn an_offline_hit_carries_a_url_exactly_when_its_text_is_stored() {
        // The kernel used to bake an index and no content, so no offline hit
        // could open and none carried a URL. It bakes the extract now, and the
        // URL follows the text: carried when there is something to read,
        // withheld when there is not — which is what demotes a row to `Hit`.
        let peek = search_offline("");
        assert!(peek.count > 0);
        let mut openable = 0;
        for i in 0..peek.count {
            let url = peek.url_at(i);
            if url.is_empty() {
                continue;
            }
            assert!(
                crate::search::body_for(url).is_some(),
                "offline hit {i} claims a URL with nothing behind it"
            );
            openable += 1;
        }
        assert!(
            openable > 0,
            "the baked corpus stores extracts, so some hit must open"
        );
    }

    #[test]
    fn a_document_opened_offline_reads_from_the_baked_extract() {
        let url = crate::search::DOCS
            .iter()
            .find(|d| !d.body.is_empty())
            .map(|d| d.url)
            .expect("a baked corpus with at least one extract");
        let page = doc_offline(url);
        assert!(!page.denied);
        assert!(page.count > 1, "an opened document showed nothing");
        // Every line fits its slot, and the last one says where the text ends.
        for i in 0..page.count {
            assert!(page.line_at(i).len() <= 83);
        }
        assert_eq!(page.line_at(page.count - 1), crate::copy::extract_only());

        // A URL the image does not store opens to nothing, and the reader
        // says so rather than showing an empty page as a document.
        let missing = doc_offline("file://nothing/here.md");
        assert_eq!(missing.count, 0);
    }

    #[test]
    fn wrapping_breaks_lines_without_dropping_words() {
        let mut page = DocPage::empty(BridgeStatus::Offline, false);
        // A word wider than a line, among ordinary prose: fixed slots in this
        // kernel cut in silence, so the check is that nothing is lost.
        let long = "capability-based-agents-with-a-very-long-hyphenated-name-that-will-not-fit-on-one-line";
        let text = "the kernel bakes an index and now an extract too";
        let mut src = [0u8; 256];
        let joined = {
            let mut n = 0;
            for part in [text, " ", long, " ", text] {
                for &b in part.as_bytes() {
                    src[n] = b;
                    n += 1;
                }
            }
            str_prefix(&src[..n])
        };
        wrap_into(&mut page, joined);
        assert!(page.count > 1, "nothing wrapped");
        let mut seen = 0usize;
        for i in 0..page.count {
            let line = page.line_at(i);
            assert!(line.len() <= 76, "line {i} is wider than the wrap");
            seen += line.bytes().filter(|b| !b.is_ascii_whitespace()).count();
        }
        let sent = joined.bytes().filter(|b| !b.is_ascii_whitespace()).count();
        assert_eq!(seen, sent, "wrapping dropped characters");
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
    fn transcribe_refuses_without_opening_com2() {
        use crate::caps::{Cap, Caps};
        let mut caps = Caps::none();
        caps.set(Cap::WorkspaceIndex, true);
        assert_eq!(
            transcribe(caps, "/tmp/demo.wav"),
            TranscribeStatus::Denied
        );
        assert!(!caps.allows(Cap::AudioTranscribe));
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

/// The portal config wire: framing safety first.
///
/// COM2 is inert in host tests (`Serial::com2()` has no base on this target),
/// so every call here degrades to the offline answer instead of hanging — which
/// is exactly the behaviour the screen depends on.
#[cfg(test)]
mod config_tests {
    use super::*;

    /// A passphrase is the normal shape of a memorable secret, and the wire
    /// carries one safely: `pass=` is the last argument of the request, so the
    /// host folds the trailing tokens back into its value.
    #[test]
    fn a_passphrase_with_spaces_is_sendable() {
        assert!(pass_frameable("correct horse battery staple"));
        assert!(pass_frameable("hunter 2"));
    }

    /// The byte that genuinely cannot be sent, and the reason it is easy to
    /// miss: the host begins a NEW argument at any token shaped `key=value`,
    /// so this secret would arrive as "correct" and fail for a reason invisible
    /// from either end.
    #[test]
    fn a_password_containing_an_equals_is_refused_rather_than_truncated() {
        assert!(!pass_frameable("correct horse=x"));
        assert_eq!(config_unlock("correct horse=x"), UnlockStatus::Unsendable);
    }

    /// Edge spaces do not survive: the host trims the line it read.
    #[test]
    fn a_password_padded_with_spaces_is_refused() {
        assert!(!pass_frameable(" leading"));
        assert!(!pass_frameable("trailing "));
        assert_eq!(config_unlock("trailing "), UnlockStatus::Unsendable);
    }

    #[test]
    fn a_password_with_a_newline_is_refused_rather_than_framed() {
        // The worst case: everything after the newline arrives as its own
        // forged CALL line.
        assert!(!pass_frameable("pass\nCALL config.portal family=teddy"));
        assert_eq!(
            config_unlock("pass\nCALL config.portal family=teddy"),
            UnlockStatus::Unsendable
        );
        assert!(!pass_frameable("pass\r"));
        assert!(!pass_frameable("pass\t"));
        assert_eq!(config_unlock("pass\r"), UnlockStatus::Unsendable);
    }

    #[test]
    fn other_unframeable_secrets_are_refused_too() {
        assert!(!pass_frameable(""), "an empty secret is not a password");
        assert!(!pass_frameable("has|pipe"), "the row separator must not pass");
        assert!(!pass_frameable("nul\0byte"));
        // Longer than the guest will frame.
        let long = "x".repeat(PASS_MAX + 1);
        assert!(!pass_frameable(&long));
        assert_eq!(config_unlock(&long), UnlockStatus::Unsendable);
    }

    #[test]
    fn ordinary_secrets_are_framed() {
        // The refusal must be narrow: real passwords still have to work.
        for ok in ["hunter2", "s3cr3t!", "a", "Tr0ub4dor&3", "~`{}[]<>,.?/"] {
            assert!(pass_frameable(ok), "refused a usable password: {ok:?}");
        }
        assert!(pass_frameable(&"x".repeat(PASS_MAX)));
    }

    #[test]
    fn an_offline_bridge_answers_instead_of_hanging() {
        // No host: every call must come back with something the screen can
        // say out loud.
        assert_eq!(config_unlock("hunter2"), UnlockStatus::Offline);
        assert_eq!(config_portal(PortalFamily::Teddy), PortalSetStatus::Offline);
        let st = config_status();
        assert!(!st.reachable);
        assert!(st.locked, "an unreachable bridge must never read as unlocked");
        assert!(!st.configured);
        assert_eq!(st.family, PortalFamily::None);
    }

    #[test]
    fn the_wire_tokens_match_the_agreed_protocol() {
        assert_eq!(PortalFamily::Teddy.wire(), "teddy");
        assert_eq!(PortalFamily::Market.wire(), "market");
        assert_eq!(PortalFamily::None.wire(), "none");
        for f in PortalFamily::ALL {
            assert_eq!(PortalFamily::from_wire(f.wire()), Some(f));
            // A family token is spliced into a CALL line unchecked, so it must
            // itself be frameable.
            assert!(pass_frameable(f.wire()), "{:?} is not wire-safe", f);
        }
        assert_eq!(PortalFamily::from_wire("nope"), None);
        assert_eq!(PortalFamily::from_wire(""), None);
    }

    #[test]
    fn the_three_choices_are_named_and_renderable() {
        assert_eq!(PortalFamily::Teddy.label(), "teddysearch.com");
        assert_eq!(PortalFamily::Market.label(), "superintelmarkets.com");
        assert_eq!(PortalFamily::None.label(), "None (offline)");
        for f in PortalFamily::ALL {
            let l = f.label();
            assert!(
                l.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII renders as '?': {l:?}"
            );
        }
    }
}

#[cfg(test)]
mod row_budget_tests {
    use super::*;

    #[test]
    fn the_number_we_ask_for_is_the_number_we_can_hold() {
        // These drifted apart once already: the request said 3 while the
        // buffer held 5, so two rows of every answer were discarded unseen.
        let asked: usize = max_rows_str().parse().expect("a number");
        assert_eq!(asked, crate::search::MAX_HITS);
        assert_eq!(
            asked,
            SearchPeek::empty(BridgeStatus::Offline, false).hits.len()
        );
    }
}
