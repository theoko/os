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

/// Longest protocol line: guest search slots (56+72+16) plus framing headroom.
const LINE_BUF: usize = 768;

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

/// Inbox peek for the home status strip (row payloads are not retained).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MailPeek {
    /// COM2 bridge down.
    Offline,
    /// Bridge up, but inbox was not read (no grant / pre-consent probe).
    Denied,
    /// `email.search` answered; `count` is ROWs received.
    Ok { count: usize },
}

impl MailPeek {
    /// Bridge reachability for nav / setup / search chrome.
    pub const fn online(self) -> bool {
        !matches!(self, Self::Offline)
    }
}

/// One key from a `ROW k=v|…` line.
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

/// One pass over a ROW for two keys (same shape as the host bridge helper).
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

/// One pass over a ROW for three keys (avoids re-splitting the line).
fn parse_row_triple<'a>(
    line: &'a str,
    ka: &str,
    kb: &str,
    kc: &str,
) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
    let mut a = None;
    let mut b = None;
    let mut c = None;
    let Some(rest) = line.strip_prefix("ROW ") else {
        return (None, None, None);
    };
    for part in rest.split('|') {
        if let Some((k, v)) = part.split_once('=') {
            if k == ka {
                a = Some(v);
            } else if k == kb {
                b = Some(v);
            } else if k == kc {
                c = Some(v);
            }
        }
    }
    (a, b, c)
}

/// Drain a typical OK / ROW* / END reply. Stops on `ERR` / `END`.
/// First line waits [`TIMEOUT_REPLY`]; later lines use [`TIMEOUT_LINE`].
/// `on_row` returns `false` to stop early. Returns whether an `ERR` was seen.
fn for_each_ok_rows(
    com2: &Serial,
    line: &mut [u8],
    max: usize,
    mut on_row: impl FnMut(&str) -> bool,
) -> bool {
    let mut saw_err = false;
    let mut first = true;
    for _ in 0..max {
        let timeout = if first { TIMEOUT_REPLY } else { TIMEOUT_LINE };
        let Some(n) = com2.read_line(line, timeout) else {
            break;
        };
        first = false;
        let resp = utf8_prefix(&line[..n]);
        if resp.starts_with("ERR ") {
            saw_err = true;
            break;
        }
        if resp == "END" {
            break;
        }
        if resp.starts_with("OK ") {
            continue;
        }
        if resp.starts_with("ROW ") && !on_row(resp) {
            break;
        }
    }
    saw_err
}

/// Ping COM2; if Online, run `f`. Otherwise return `offline`.
fn when_online<T>(offline: T, f: impl FnOnce(&Serial, &mut [u8]) -> T) -> T {
    let mut line = [0u8; LINE_BUF];
    let com2 = Serial::com2();
    com2.init();
    if !ping_bridge(&com2, &mut line) {
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
/// Without [`crate::caps::Cap::EmailSearch`] (including [`crate::caps::Caps::none`]
/// before consent) this only PINGs — Online → [`MailPeek::Denied`], never
/// `CALL email.search` (which would persist inbox results on the host).
pub fn fetch_mail_peek(caps: crate::caps::Caps) -> MailPeek {
    when_online(MailPeek::Offline, |com2, line| {
        if !caps.allows(crate::caps::Cap::EmailSearch) {
            // Bridge is up, but this guest was not granted inbox read.
            return MailPeek::Denied;
        }

        // Bridge defaults: q=in:inbox, max=3 (guest mail-peek budget).
        com2.write_str("CALL email.search\n");

        // Count ROWs actually received. ERR is not an empty inbox — same
        // Denied path as a missing grant.
        let mut count = 0usize;
        let saw_err = for_each_ok_rows(com2, line, 16, |_| {
            count += 1;
            true
        });
        if saw_err {
            MailPeek::Denied
        } else {
            MailPeek::Ok { count }
        }
    })
}

/// How a framed CALL finished (`doc.read`, `search.query`).
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
    /// Copied from the search hit (not on the wire) — guest title slot size.
    title: [u8; Self::TITLE_CHARS],
    lines: [[u8; Self::LINE_CHARS]; Self::MAX],
}

impl DocPage {
    pub(crate) const MAX: usize = 18;
    /// Matches bridge / `SearchView::Row` title width.
    pub(crate) const TITLE_CHARS: usize = 56;
    /// Matches bridge `search::LINE_WIDTH` for `ROW line=`.
    pub(crate) const LINE_CHARS: usize = 78;

    pub const fn empty(outcome: DocOutcome) -> Self {
        Self {
            outcome,
            count: 0,
            title: [0; Self::TITLE_CHARS],
            lines: [[0; Self::LINE_CHARS]; Self::MAX],
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
        // Bridge default lines= matches DocPage::MAX; guest still caps ROWs locally.
        com2.write_str("CALL doc.read url=");
        com2.write_str(url);
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let mut page = DocPage::empty(DocOutcome::Ok);
        let saw_err = for_each_ok_rows(com2, line, 40, |resp| {
            if page.count >= DocPage::MAX {
                return false;
            }
            let text = parse_row_field(resp, "line").unwrap_or("");
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
/// Returns `true` when a purge CALL got a framed OK (not `ERR` / offline).
pub fn forget(cap: crate::caps::Cap) -> bool {
    let Some(tool) = cap.forget_tool() else {
        return false;
    };
    when_online(false, |com2, line| {
        com2.write_str("CALL ");
        com2.write_str(tool);
        com2.write_str("\n");
        // Drain OK/ROW/END (stop on ERR) so the next call starts clean.
        !for_each_ok_rows(com2, line, 8, |_| true)
    })
}

/// List playbooks via `CALL skills.list`. Offline / ERR → ISO builtins.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    when_online(crate::skills::SkillPeek::from_builtin(), |com2, line| {
        com2.write_str("CALL skills.list\n");

        let mut peek = crate::skills::SkillPeek::empty();
        let saw_err = for_each_ok_rows(com2, line, 24, |resp| {
            let (name, desc) = parse_row_pair(resp, "name", "desc");
            peek.push(name.unwrap_or("?"), desc.unwrap_or(""))
        });
        if saw_err {
            // Failed CALL — show ISO defaults; from_bridge stays false.
            crate::skills::SkillPeek::from_builtin()
        } else {
            // Framed OK: keep Listed even at count==0 (do not pretend offline).
            peek
        }
    })
}

fn ping_bridge(com2: &Serial, line: &mut [u8]) -> bool {
    for _ in 0..64 {
        if com2.try_read_byte().is_none() {
            break;
        }
    }
    com2.write_str("PING\n");
    let Some(n) = com2.read_line(line, TIMEOUT_PING) else {
        return false;
    };
    utf8_prefix(&line[..n]).starts_with("OK pong")
}

/// Run `search.query`. `q` must be ASCII without spaces (use `-`).
///
/// Caller must hold [`crate::caps::Cap::SearchQuery`]. Scope flags
/// (`email=1`, `files=1`, …) still follow the rest of `caps`.
/// Invokes `on_hit(title, url, cat)` for each ROW (stop early by returning `false`).
/// Offline → baked-index fallback; `Err` → denied empty state; `Ok` even with
/// zero hits so the UI can tell "no matches" from "no bridge".
pub(crate) fn fetch_search_rows(
    caps: crate::caps::Caps,
    q: &str,
    mut on_hit: impl FnMut(&str, &str, &str) -> bool,
) -> DocOutcome {
    // Offline: UI falls back to the baked index via SearchView::fill_local.
    when_online(DocOutcome::Offline, |com2, line| {
        // CALL search.query q=… [email=1] [files=1] [audio=1]
        // Bridge default k= matches MAX_HITS. Email graph is opt-in per call —
        // holding search.query alone must not reach mail content.
        com2.write_str("CALL search.query q=");
        com2.write_str(q);
        if caps.allows(crate::caps::Cap::EmailSearch) {
            com2.write_str(" email=1");
        }
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let saw_err = for_each_ok_rows(com2, line, 16, |resp| {
            let (title, url, cat) = parse_row_triple(resp, "title", "url", "cat");
            // Caller enforces MAX_HITS (returns false to stop).
            on_hit(
                title.unwrap_or("?"),
                url.unwrap_or(""),
                cat.unwrap_or(""),
            )
        });
        if saw_err {
            DocOutcome::Err
        } else {
            DocOutcome::Ok
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_row_field_extracts_keys() {
        let search = "ROW title=MCP overview|cat=docs|url=https://example/mcp";
        assert_eq!(
            parse_row_triple(search, "title", "url", "cat"),
            (
                Some("MCP overview"),
                Some("https://example/mcp"),
                Some("docs"),
            )
        );

        let skill = "ROW name=email-triage|desc=Inbox via MCP email";
        assert_eq!(
            parse_row_pair(skill, "name", "desc"),
            (Some("email-triage"), Some("Inbox via MCP email"))
        );
        // Single-key extract (doc.read uses `line=` this way).
        assert_eq!(
            parse_row_field("ROW line=hello world", "line"),
            Some("hello world")
        );
    }
}
