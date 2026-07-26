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

/// One pass over a ROW for up to two keys (avoids re-splitting the line).
/// Pass `""` for `kb` when only one key is needed.
fn parse_row_pair<'a>(line: &'a str, ka: &str, kb: &str) -> (Option<&'a str>, Option<&'a str>) {
    let mut a = None;
    let mut b = None;
    let Some(rest) = line.strip_prefix("ROW ") else {
        return (None, None);
    };
    for part in rest.split('|') {
        if let Some((k, v)) = part.split_once('=') {
            if k == ka {
                a = Some(v);
            } else if k == kb {
                b = Some(v);
            }
        }
    }
    (a, b)
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

/// Drain a typical OK / ROW* / END reply. Stops on `ERR` / `END`.
/// `on_row` returns `false` to stop early. Returns `(saw_err, n)` where `n`
/// is the `n=` count from the OK header (0 if absent).
fn for_each_ok_rows(
    com2: &Serial,
    line: &mut [u8],
    max: usize,
    mut on_row: impl FnMut(&str) -> bool,
) -> (bool, usize) {
    let mut saw_err = false;
    let mut ok_n = 0usize;
    for_each_reply(com2, line, max, |resp| {
        if resp.starts_with("ERR ") {
            saw_err = true;
            return false;
        }
        if resp == "END" {
            return false;
        }
        if resp.starts_with("OK ") {
            ok_n = parse_ok_n(resp);
            return true;
        }
        if resp.starts_with("ROW ") {
            return on_row(resp);
        }
        true
    });
    (saw_err, ok_n)
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

        // Count from OK `n=`; ROWs are drained for wire hygiene only.
        let mut peek = MailPeek::empty(BridgeStatus::Online);
        let (_, n) = for_each_ok_rows(com2, line, 16, |_| true);
        peek.count = n;
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

/// How `doc.read` finished (replaces parallel `status` + `denied` bits).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DocOutcome {
    Offline,
    /// Bridge returned `ERR` (grant miss, not found, …).
    Err,
    /// Bridge returned a framed OK (zero or more lines).
    Ok,
}

/// Lines of a document, for the reader.
pub struct DocPage {
    pub(crate) outcome: DocOutcome,
    pub(crate) count: usize,
    title: [u8; 72],
    lines: [[u8; 84]; Self::MAX],
}

impl DocPage {
    pub(crate) const MAX: usize = 18;

    pub const fn empty(outcome: DocOutcome) -> Self {
        Self {
            outcome,
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
/// to read it by knowing its URL. `title` is the search-hit label (not on the wire).
pub fn fetch_doc(caps: crate::caps::Caps, url: &str, title: &str) -> DocPage {
    let mut page = when_online(DocPage::empty(DocOutcome::Offline), |com2, line| {
        com2.write_str("CALL doc.read url=");
        com2.write_str(url);
        com2.write_str(" lines=");
        com2.write_u64(DocPage::MAX as u64);
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut page = DocPage::empty(DocOutcome::Ok);
        let (saw_err, _) = for_each_ok_rows(com2, line, 40, |resp| {
            if page.count >= DocPage::MAX {
                return false;
            }
            let text = parse_row_pair(resp, "line", "").0.unwrap_or("");
            copy_field(&mut page.lines[page.count], text);
            page.count += 1;
            true
        });
        if saw_err {
            page.outcome = DocOutcome::Err;
        }
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
        // Drain OK/ROW/END (stop on ERR) so the next call starts clean.
        let _ = for_each_ok_rows(com2, line, 8, |_| true);
    })
}

/// Liveness only: PING the bridge without reading any mailbox.
///
/// Used before the user has consented on the Capabilities step. Calling
/// `fetch_mail_peek` there would read — and, since the bridge indexes results,
/// *persist* — the inbox before anyone agreed to it.
pub fn probe_bridge() -> BridgeStatus {
    let mut line = [0u8; LINE_BUF];
    let com2 = Serial::com2();
    com2.init();
    ping_bridge(&com2, &mut line)
}

/// List playbooks via `CALL skills.list`. Offline → builtins baked into the ISO.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    when_online(crate::skills::SkillPeek::from_builtin(), |com2, line| {
        com2.write_str("CALL skills.list\n");

        let mut peek = crate::skills::SkillPeek::empty();
        peek.from_bridge = true;
        let _ = for_each_ok_rows(com2, line, 24, |resp| {
            let (name, desc) = parse_row_pair(resp, "name", "desc");
            peek.push(name.unwrap_or("?"), desc.unwrap_or(""))
        })
        .0;

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
/// Invokes `on_hit(title, url)` for each ROW (stop early by returning `false`).
/// Returns `false` when the bridge is offline; `true` when it answered
/// (even with zero hits — so the UI can tell "no matches" from "no bridge").
pub(crate) fn fetch_search_rows(
    caps: crate::caps::Caps,
    q: &str,
    mut on_hit: impl FnMut(&str, &str) -> bool,
) -> bool {
    // Offline: UI falls back to the baked index via SearchView::fill_local.
    when_online(false, |com2, line| {
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

        let _ = for_each_ok_rows(com2, line, 16, |resp| {
            let (title, url) = parse_row_pair(resp, "title", "url");
            // Caller enforces MAX_HITS (returns false to stop).
            on_hit(title.unwrap_or("?"), url.unwrap_or(""))
        })
        .0;
        true
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
        assert_eq!(
            parse_row_pair(search, "title", "url"),
            (Some("MCP overview"), Some("https://example/mcp"))
        );

        let skill = "ROW name=email-triage|src=default|desc=Inbox via MCP email";
        assert_eq!(
            parse_row_pair(skill, "name", "desc"),
            (Some("email-triage"), Some("Inbox via MCP email"))
        );
        // Unread keys must not disturb neighbors.
        assert_eq!(parse_row_pair(skill, "src", "").0, Some("default"));
    }
}
