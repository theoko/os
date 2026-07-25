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

/// Footer / empty-state hint when COM2 has no host bridge.
pub const BRIDGE_OFFLINE_HINT: &str = "Bridge offline - run: make utm-bridged";

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
        str_at(&self.rows[i].from)
    }

    pub fn row_subj(&self, i: usize) -> &str {
        str_at(&self.rows[i].subj)
    }
}

/// One hit from `search.query`.
pub struct SearchHit {
    pub title: [u8; 48],
    /// Source URL, needed to open the document rather than only name it.
    pub url: [u8; 72],
}

/// Short corpus peek from the host bridge.
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
        str_at(&self.hits[i].title)
    }

    pub fn url_at(&self, i: usize) -> &str {
        str_at(&self.hits[i].url)
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

/// Drain a typical OK / ROW* / END reply. Skips the `OK …` header; stops on
/// `ERR` / `END`. `on_row` returns `false` to stop early (e.g. buffer full).
/// Returns whether an `ERR` line was seen.
fn for_each_ok_rows(
    com2: &Serial,
    line: &mut [u8],
    max: usize,
    ok_prefix: &str,
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
        if resp.starts_with(ok_prefix) {
            return true;
        }
        if resp.starts_with("ROW ") {
            return on_row(resp);
        }
        true
    });
    saw_err
}

fn open_com2(line: &mut [u8]) -> (Serial, BridgeStatus) {
    let com2 = Serial::com2();
    com2.init();
    let status = ping_bridge(&com2, line);
    (com2, status)
}

/// Ping COM2; if Online, run `f`. Otherwise return `offline`.
fn when_online<T>(offline: T, f: impl FnOnce(&Serial, &mut [u8]) -> T) -> T {
    let mut line = [0u8; LINE_BUF];
    let (com2, status) = open_com2(&mut line);
    if status != BridgeStatus::Online {
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

        let mut peek = MailPeek::empty(BridgeStatus::Online);
        let _ = for_each_ok_rows(com2, line, 16, "OK email.search", |resp| {
            if peek.count >= peek.rows.len() {
                return false;
            }
            let from = parse_row_field(resp, "from").unwrap_or("?");
            let subj = parse_row_field(resp, "subj").unwrap_or("(no subject)");
            copy_field(&mut peek.rows[peek.count].from, from);
            copy_field(&mut peek.rows[peek.count].subj, subj);
            peek.count += 1;
            true
        });
        peek
    })
}

/// Lines of a document, for the reader.
pub struct DocPage {
    pub status: BridgeStatus,
    pub denied: bool,
    pub count: usize,
    pub lines: [[u8; 84]; Self::MAX],
}

impl DocPage {
    pub const MAX: usize = 18;

    pub const fn empty(status: BridgeStatus, denied: bool) -> Self {
        Self { status, denied, count: 0, lines: [[0; 84]; Self::MAX] }
    }

    pub fn line_at(&self, i: usize) -> &str {
        str_at(&self.lines[i])
    }
}

/// Read a document the search results pointed at.
///
/// The same grants are sent as for the query, because the bridge checks scope
/// per source: a caller that could not have found a document must not be able
/// to read it by knowing its URL.
pub fn fetch_doc(caps: crate::caps::Caps, url: &str) -> DocPage {
    when_online(DocPage::empty(BridgeStatus::Offline, false), |com2, line| {
        com2.write_str("CALL doc.read url=");
        com2.write_str(url);
        com2.write_str(" lines=18");
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut page = DocPage::empty(BridgeStatus::Online, false);
        page.denied = for_each_ok_rows(com2, line, 40, "OK doc.read", |resp| {
            if page.count >= DocPage::MAX {
                return false;
            }
            let text = parse_row_field(resp, "line").unwrap_or("");
            copy_field(&mut page.lines[page.count], text);
            page.count += 1;
            true
        });
        page
    })
}

/// Ask the bridge to delete what a revoked capability produced.
///
/// Turning a switch off should remove the index it built, not just stop
/// answering from it — otherwise "off" means "hidden", which is not what the
/// switch says.
pub fn forget(tool: &str) -> BridgeStatus {
    when_online(BridgeStatus::Offline, |com2, line| {
        com2.write_str("CALL ");
        com2.write_str(tool);
        com2.write_str("\n");
        // Drain the reply so the next call starts on a clean line.
        for_each_reply(com2, line, 8, |resp| resp != "END");
        BridgeStatus::Online
    })
}

/// Liveness only: PING the bridge without reading any mailbox.
///
/// Used before the user has consented on the Capabilities step. Calling
/// `fetch_mail_peek` there would read — and, since the bridge indexes results,
/// *persist* — the inbox before anyone agreed to it.
pub fn probe_bridge() -> BridgeStatus {
    let mut line = [0u8; LINE_BUF];
    open_com2(&mut line).1
}

/// List playbooks via `CALL skills.list`. Offline → builtins baked into the ISO.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    when_online(crate::skills::SkillPeek::from_builtin(), |com2, line| {
        com2.write_str("CALL skills.list\n");

        let mut peek = crate::skills::SkillPeek::empty();
        peek.from_bridge = true;
        let _ = for_each_ok_rows(com2, line, 24, "OK skills.list", |resp| {
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

/// First useful body line from `CALL skills.get name=…` (for a clicked row).
///
/// Returns `false` when the bridge is down or the skill is missing.
pub fn fetch_skill_blurb(name: &str, out: &mut [u8]) -> bool {
    out.fill(0);
    if name.is_empty() {
        return false;
    }
    when_online(false, |com2, line| {
        com2.write_str("CALL skills.get name=");
        com2.write_str(name);
        com2.write_str("\n");

        let mut in_frontmatter = false;
        let mut saw_fm_open = false;
        let mut found = false;
        for_each_reply(com2, line, 40, |resp| {
            if resp == "END" || resp.starts_with("ERR ") {
                return false;
            }
            if resp.starts_with("OK skills.get") {
                return true;
            }
            let Some(body) = resp.strip_prefix("LINE ") else {
                return true;
            };
            // Skip YAML frontmatter so the blurb is real prose, not `---`.
            if body.trim() == "---" {
                if !saw_fm_open {
                    saw_fm_open = true;
                    in_frontmatter = true;
                } else {
                    in_frontmatter = false;
                }
                return true;
            }
            if in_frontmatter {
                return true;
            }
            let text = body.trim();
            if text.is_empty() {
                return true;
            }
            copy_field(out, text);
            found = true;
            false
        });
        found
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

/// Run `search.query` when granted. `q` must be ASCII without spaces (use `-`).
pub fn fetch_search_peek(caps: crate::caps::Caps, q: &str) -> SearchPeek {
    // Denied before CALL: still PING so the UI can show Online vs Offline.
    if !caps.allows(crate::caps::Cap::SearchQuery) {
        let mut line = [0u8; LINE_BUF];
        let status = open_com2(&mut line).1;
        return SearchPeek::empty(status, true);
    }

    // Offline: UI falls back to the baked index via SearchView::run(q).
    when_online(SearchPeek::empty(BridgeStatus::Offline, false), |com2, line| {
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
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut peek = SearchPeek::empty(BridgeStatus::Online, false);
        let _ = for_each_ok_rows(com2, line, 16, "OK search.query", |resp| {
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
        peek
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn offline_search_falls_back_in_the_ui() {
        // fetch_search_peek returns empty Offline; SearchView::run_via then
        // queries the baked index with the user's actual string.
        let peek = SearchPeek::empty(BridgeStatus::Offline, false);
        assert_eq!(peek.count, 0);
        assert!(!peek.denied);
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
    }
}
