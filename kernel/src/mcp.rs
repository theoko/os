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

/// Longest protocol line: guest search slots plus framing headroom.
const LINE_BUF: usize = 768;

/// Bridge default `email.search max=` (guest omits the arg).
const MAIL_PEEK_MAX: usize = 3;

/// OK line + END-break allowance around ROW drains in [`for_each_ok_rows`].
const FRAMED_PAD: usize = 2;

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
    /// Bridge up. `inbox: None` = no grant / pre-consent / framed ERR
    /// (not an empty inbox). `Some(n)` = ROW count from `email.search`.
    Online { inbox: Option<usize> },
}

impl MailPeek {
    /// Bridge reachability for nav / setup / search chrome.
    pub const fn online(self) -> bool {
        matches!(self, Self::Online { .. })
    }
}

/// One pass over a `ROW k=v|…` line for `N` keys (avoids re-splitting).
fn parse_row<'a, const N: usize>(line: &'a str, keys: [&str; N]) -> [Option<&'a str>; N] {
    let mut out = [None; N];
    let Some(rest) = line.strip_prefix("ROW ") else {
        return out;
    };
    for part in rest.split('|') {
        if let Some((k, v)) = part.split_once('=') {
            for (i, key) in keys.iter().enumerate() {
                if k == *key {
                    out[i] = Some(v);
                }
            }
        }
    }
    out
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
/// before consent) this only PINGs — Online with `inbox: None`, never
/// `CALL email.search` (gog / mock would still hit the host mailbox).
pub fn fetch_mail_peek(caps: crate::caps::Caps) -> MailPeek {
    when_online(MailPeek::Offline, |com2, line| {
        if !caps.allows(crate::caps::Cap::EmailSearch) {
            return MailPeek::Online { inbox: None };
        }

        // Bridge defaults: q=in:inbox, max=MAIL_PEEK_MAX.
        com2.write_str("CALL email.search\n");

        // Count ROWs actually received. ERR is not an empty inbox.
        let mut count = 0usize;
        let saw_err = for_each_ok_rows(com2, line, MAIL_PEEK_MAX + FRAMED_PAD, |_| {
            count += 1;
            true
        });
        if saw_err {
            MailPeek::Online { inbox: None }
        } else {
            MailPeek::Online {
                inbox: Some(count),
            }
        }
    })
}

/// How a framed CALL finished (`doc.read`, `search.query`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocOutcome {
    Offline,
    /// Bridge returned framed `ERR` (not found, host-side refuse, …).
    Err,
    /// Bridge returned a framed OK (zero or more lines).
    Ok,
}

/// Lines of a document, for the reader.
pub struct DocPage {
    pub(crate) outcome: DocOutcome,
    pub(crate) count: usize,
    /// Copied from the search hit (not on the wire).
    title: [u8; crate::search::TITLE_CHARS],
    lines: [[u8; Self::LINE_CHARS]; Self::MAX],
}

impl DocPage {
    pub(crate) const MAX: usize = 18;
    /// Matches bridge `search::LINE_CHARS` for `ROW line=`.
    pub(crate) const LINE_CHARS: usize = 78;

    pub const fn empty(outcome: DocOutcome) -> Self {
        Self {
            outcome,
            count: 0,
            title: [0; crate::search::TITLE_CHARS],
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
        let saw_err = for_each_ok_rows(com2, line, DocPage::MAX + FRAMED_PAD, |resp| {
            if page.count >= DocPage::MAX {
                return false;
            }
            // Missing `line=` is skipped; empty value (`ROW line=`) is a blank.
            let [Some(text)] = parse_row(resp, ["line"]) else {
                return true;
            };
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
        // Framed forget is OK + END (stop on ERR) so the next call starts clean.
        !for_each_ok_rows(com2, line, FRAMED_PAD, |_| true)
    })
}

/// List playbooks via `CALL skills.list`. Offline / ERR → ISO builtins.
pub fn fetch_skill_peek() -> crate::skills::SkillPeek {
    when_online(crate::skills::SkillPeek::from_builtins(), |com2, line| {
        com2.write_str("CALL skills.list\n");

        let mut peek = crate::skills::SkillPeek::empty_live();
        // OK + up to MAX_LISTED ROWs (+ END breaks); push false also stops early.
        let saw_err = for_each_ok_rows(com2, line, crate::skills::MAX_LISTED + FRAMED_PAD, |resp| {
            let [Some(name), desc] = parse_row(resp, ["name", "desc"]) else {
                return true;
            };
            peek.push(name, desc.unwrap_or(""))
        });
        if saw_err {
            // Failed CALL — show ISO defaults.
            crate::skills::SkillPeek::from_builtins()
        } else {
            // Framed OK: keep live even at count==0 (do not pretend offline).
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
/// (`files=1`, `audio=1`) still follow the rest of `caps`.
/// Invokes `on_hit(title, url, cat)` for each ROW (stop early by returning `false`).
/// Offline → UI may fill the baked index; `Err`/`Ok` empty stay empty so the
/// UI can tell TEDDY / "no matches" from "no bridge".
pub(crate) fn fetch_search_rows(
    caps: crate::caps::Caps,
    q: &str,
    mut on_hit: impl FnMut(&str, &str, &str) -> bool,
) -> DocOutcome {
    // Offline: SearchView::fill_if_offline may pad the baked index.
    when_online(DocOutcome::Offline, |com2, line| {
        // CALL search.query q=… [files=1] [audio=1]
        // Bridge default k= matches MAX_HITS. Mail is peek-only (no search hits).
        com2.write_str("CALL search.query q=");
        com2.write_str(q);
        write_scope_flags(com2, caps);
        com2.write_str("\n");

        let saw_err = for_each_ok_rows(com2, line, crate::search::MAX_HITS + FRAMED_PAD, |resp| {
            // title+url required; cat optional chip.
            let [Some(title), Some(url), cat] = parse_row(resp, ["title", "url", "cat"]) else {
                return true;
            };
            // Caller enforces MAX_HITS (returns false to stop).
            on_hit(title, url, cat.unwrap_or(""))
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
    fn parse_row_extracts_keys() {
        let search = "ROW title=MCP overview|cat=docs|url=https://example/mcp";
        assert_eq!(
            parse_row(search, ["title", "url", "cat"]),
            [
                Some("MCP overview"),
                Some("https://example/mcp"),
                Some("docs"),
            ]
        );

        let skill = "ROW name=email-triage|desc=Inbox via MCP email";
        assert_eq!(
            parse_row(skill, ["name", "desc"]),
            [Some("email-triage"), Some("Inbox via MCP email")]
        );
        // Single-key extract (doc.read uses `line=` this way).
        assert_eq!(
            parse_row("ROW line=hello world", ["line"]),
            [Some("hello world")]
        );
        // Missing required key stays None (call sites skip inventing "?").
        assert_eq!(parse_row("ROW url=https://x", ["title", "url"]), [None, Some("https://x")]);
    }
}
