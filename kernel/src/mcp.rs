//! Guest MCP client over COM2 (host bridge).

use crate::serial::Serial;
use crate::skills::{copy_field, str_at, utf8_prefix};

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

/// Make-target tip shared by footer and empty-state offline copy.
macro_rules! bridge_offline_tip {
    () => {
        "Bridge offline - run: make utm-bridged"
    };
}

/// Footer / status-card hint when COM2 has no host bridge.
pub(crate) const BRIDGE_OFFLINE_HINT: &str = bridge_offline_tip!();

/// Search empty-state line when the bridge is down (same tip, prefixed).
pub(crate) const NO_MATCHES_BRIDGE_OFFLINE: &str =
    concat!("No matches. ", bridge_offline_tip!());

/// Inbox liveness + unread count for the home status strip.
/// Row payloads are not retained — the home UI only shows a count.
pub struct MailPeek {
    pub status: BridgeStatus,
    pub(crate) count: usize,
}

impl MailPeek {
    pub const fn empty(status: BridgeStatus) -> Self {
        Self { status, count: 0 }
    }
}

/// One hit from `search.query`.
struct SearchHit {
    title: [u8; 48],
    /// Source URL, needed to open the document rather than only name it.
    url: [u8; 72],
}

/// Short corpus peek from the host bridge.
///
/// Callers must hold [`crate::caps::Cap::SearchQuery`] before calling
/// [`fetch_search_peek`]; the cap gate lives in the UI, not here.
pub(crate) struct SearchPeek {
    pub(crate) count: usize,
    hits: [SearchHit; crate::search::MAX_HITS],
}

impl SearchPeek {
    const fn empty() -> Self {
        const EMPTY: SearchHit = SearchHit { title: [0; 48], url: [0; 72] };
        Self {
            count: 0,
            hits: [EMPTY; crate::search::MAX_HITS],
        }
    }

    pub(crate) fn at(&self, i: usize) -> (&str, &str) {
        (str_at(&self.hits[i].title), str_at(&self.hits[i].url))
    }
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

/// Walk COM2 reply lines after a CALL. First line waits [`TIMEOUT_REPLY`];
/// later lines use [`TIMEOUT_LINE`]. Callback returns `false` to stop.
fn for_each_reply(
    com2: &Serial,
    line: &mut [u8],
    max: usize,
    mut f: impl FnMut(&str) -> bool,
) {
    let mut first = true;
    for _ in 0..max {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(line, timeout) else {
            break;
        };
        first = false;
        if !f(utf8_prefix(&line[..n])) {
            break;
        }
    }
}

/// Drain a typical OK / ROW* / END reply. Skips any `OK …` header; stops on
/// `ERR` / `END`. `on_row` returns `false` to stop early (e.g. buffer full).
/// Returns whether an `ERR` line was seen.
fn for_each_ok_rows(
    com2: &Serial,
    line: &mut [u8],
    max: usize,
    mut on_row: impl FnMut(&str) -> bool,
) -> bool {
    let mut saw_err = false;
    for_each_reply(com2, line, max, |resp| {
        if resp.starts_with("ERR ") {
            saw_err = true;
            return false;
        }
        if resp == "END" {
            return false;
        }
        if resp.starts_with("OK ") {
            return true;
        }
        if resp.starts_with("ROW ") {
            return on_row(resp);
        }
        true
    });
    saw_err
}

/// Ping COM2; if Online, run `f`. Otherwise return `offline`.
fn when_online<T>(offline: T, f: impl FnOnce(&Serial, &mut [u8]) -> T) -> T {
    let mut line = [0u8; LINE_BUF];
    let com2 = Serial::com2();
    com2.init();
    if ping_bridge(&com2, &mut line) != BridgeStatus::Online {
        offline
    } else {
        f(&com2, &mut line)
    }
}

/// Opt-in scope flags shared by `doc.read` / `search.query`.
fn write_scope_flags(com2: &Serial, caps: crate::caps::Caps) {
    if caps.allows(crate::caps::Cap::WorkspaceIndex) {
        com2.write_str(" files=1");
    }
    if caps.allows(crate::caps::Cap::AudioTranscribe) {
        com2.write_str(" audio=1");
    }
}

/// Probe the host bridge and optionally fetch a short inbox peek.
///
/// `email.search` is refused when `caps` does not grant [`crate::caps::Cap::EmailSearch`].
pub fn fetch_mail_peek(caps: crate::caps::Caps) -> MailPeek {
    when_online(MailPeek::empty(BridgeStatus::Offline), |com2, line| {
        if !caps.allows(crate::caps::Cap::EmailSearch) {
            // Bridge is up, but this guest was not granted inbox read.
            return MailPeek::empty(BridgeStatus::Online);
        }

        com2.write_str("CALL email.search q=in:inbox max=3\n");

        // Count comes from the OK header (`n=`); drain ROW/END for wire hygiene.
        let mut peek = MailPeek::empty(BridgeStatus::Online);
        for_each_reply(com2, line, 16, |resp| {
            if resp.starts_with("ERR ") || resp == "END" {
                return false;
            }
            if resp.starts_with("OK email.search") {
                peek.count = parse_ok_n(resp);
            }
            true
        });
        peek
    })
}

/// Digits after `n=` on an OK header (`OK email.search n=3`).
fn parse_ok_n(line: &str) -> usize {
    let Some((_, after)) = line.split_once("n=") else {
        return 0;
    };
    let mut n = 0usize;
    for &b in after.as_bytes() {
        if !b.is_ascii_digit() {
            break;
        }
        n = n.saturating_mul(10).saturating_add((b - b'0') as usize);
    }
    n
}

/// Lines of a document, for the reader.
pub struct DocPage {
    pub(crate) status: BridgeStatus,
    pub(crate) denied: bool,
    pub(crate) count: usize,
    title: [u8; 72],
    lines: [[u8; 84]; Self::MAX],
}

impl DocPage {
    pub(crate) const MAX: usize = 18;

    pub const fn empty(status: BridgeStatus) -> Self {
        Self {
            status,
            denied: false,
            count: 0,
            title: [0; 72],
            lines: [[0; 84]; Self::MAX],
        }
    }

    pub(crate) fn title(&self) -> &str {
        str_at(&self.title)
    }

    pub(crate) fn line_at(&self, i: usize) -> &str {
        str_at(&self.lines[i])
    }
}

/// Read a document the search results pointed at.
///
/// The same grants are sent as for the query, because the bridge checks scope
/// per source: a caller that could not have found a document must not be able
/// to read it by knowing its URL.
pub fn fetch_doc(caps: crate::caps::Caps, title: &str, url: &str) -> DocPage {
    let mut page = when_online(DocPage::empty(BridgeStatus::Offline), |com2, line| {
        com2.write_str("CALL doc.read url=");
        com2.write_str(url);
        com2.write_str(" lines=");
        com2.write_u64(DocPage::MAX as u64);
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut page = DocPage::empty(BridgeStatus::Online);
        page.denied = for_each_ok_rows(com2, line, 40, |resp| {
            if page.count >= DocPage::MAX {
                return false;
            }
            let text = parse_row_field(resp, "line").unwrap_or("");
            copy_field(&mut page.lines[page.count], text);
            page.count += 1;
            true
        });
        page
    });
    copy_field(&mut page.title, title);
    page
}

/// Ask the bridge to delete what a revoked capability produced.
///
/// Turning a switch off should remove the index it built, not just stop
/// answering from it — otherwise "off" means "hidden", which is not what the
/// switch says.
pub fn forget(tool: &str) {
    when_online((), |com2, line| {
        com2.write_str("CALL ");
        com2.write_str(tool);
        com2.write_str("\n");
        // Drain the reply so the next call starts on a clean line.
        for_each_reply(com2, line, 8, |resp| resp != "END");
    })
}

/// Liveness only: PING the bridge without reading any mailbox.
///
/// Used before the user has consented on the Capabilities step. Calling
/// `fetch_mail_peek` there would read — and, since the bridge indexes results,
/// *persist* — the inbox before anyone agreed to it.
pub fn probe_bridge() -> BridgeStatus {
    when_online(BridgeStatus::Offline, |_, _| BridgeStatus::Online)
}

/// List playbooks via `CALL skills.list`. Offline → builtins baked into the ISO.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    when_online(crate::skills::SkillPeek::from_builtin(), |com2, line| {
        com2.write_str("CALL skills.list\n");

        let mut peek = crate::skills::SkillPeek::empty();
        peek.from_bridge = true;
        let _ = for_each_ok_rows(com2, line, 24, |resp| {
            let name = parse_row_field(resp, "name").unwrap_or("?");
            let desc = parse_row_field(resp, "desc").unwrap_or("");
            peek.push(name, desc)
        });

        if peek.count == 0 {
            // Bridge answered but listed nothing — still show ISO defaults.
            crate::skills::SkillPeek::from_builtin()
        } else {
            peek
        }
    })
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
    let resp = utf8_prefix(&line[..n]);
    if resp.starts_with("OK pong") {
        BridgeStatus::Online
    } else {
        BridgeStatus::Offline
    }
}

/// Run `search.query`. `q` must be ASCII without spaces (use `-`).
///
/// Caller must hold [`crate::caps::Cap::SearchQuery`]. Scope flags
/// (`email=1`, `files=1`, …) still follow the rest of `caps`.
/// Returns [`None`] when the bridge is offline.
pub(crate) fn fetch_search_peek(caps: crate::caps::Caps, q: &str) -> Option<SearchPeek> {
    // Offline: UI falls back to the baked index via SearchView::fill_local.
    when_online(None, |com2, line| {
        // CALL search.query q=… k=N [email=1]
        //
        // The email graph is opt-in per call on the bridge. Ask for it only when
        // the user granted email.search at setup: holding search.query alone must
        // not reach mail content.
        com2.write_str("CALL search.query q=");
        com2.write_str(q);
        com2.write_str(" k=");
        com2.write_u64(crate::search::MAX_HITS as u64);
        if caps.allows(crate::caps::Cap::EmailSearch) {
            com2.write_str(" email=1");
        }
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut peek = SearchPeek::empty();
        let _ = for_each_ok_rows(com2, line, 16, |resp| {
            if peek.count >= peek.hits.len() {
                return false;
            }
            let title = parse_row_field(resp, "title").unwrap_or("?");
            copy_field(&mut peek.hits[peek.count].title, title);
            copy_field(
                &mut peek.hits[peek.count].url,
                parse_row_field(resp, "url").unwrap_or(""),
            );
            peek.count += 1;
            true
        });
        Some(peek)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok_count() {
        assert_eq!(parse_ok_n("OK email.search n=3"), 3);
        assert_eq!(parse_ok_n("OK email.search n=0"), 0);
        assert_eq!(parse_ok_n("OK email.search"), 0);
    }

    #[test]
    fn parse_row_field_extracts_keys() {
        let search = "ROW title=MCP overview|url=https://example/mcp";
        assert_eq!(parse_row_field(search, "title"), Some("MCP overview"));
        assert_eq!(parse_row_field(search, "url"), Some("https://example/mcp"));

        let skill = "ROW name=email-triage|src=default|desc=Inbox via MCP email";
        assert_eq!(parse_row_field(skill, "name"), Some("email-triage"));
        assert_eq!(parse_row_field(skill, "desc"), Some("Inbox via MCP email"));
        // Unread keys must not disturb neighbors.
        assert_eq!(parse_row_field(skill, "src"), Some("default"));
    }
}
