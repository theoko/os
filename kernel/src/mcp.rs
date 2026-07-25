//! Guest MCP client over COM2 (host bridge).

use crate::serial::Serial;

const TIMEOUT_PING: u32 = 80_000;
const TIMEOUT_LINE: u32 = 200_000;

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
        core::str::from_utf8(trim_buf(&self.rows[i].from)).unwrap_or("")
    }

    pub fn row_subj(&self, i: usize) -> &str {
        core::str::from_utf8(trim_buf(&self.rows[i].subj)).unwrap_or("")
    }
}

/// One hit from `search.query`.
pub struct SearchHit {
    pub title: [u8; 48],
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
        const EMPTY: SearchHit = SearchHit { title: [0; 48] };
        Self {
            status,
            denied,
            count: 0,
            hits: [EMPTY; 3],
        }
    }

    pub fn title_at(&self, i: usize) -> &str {
        core::str::from_utf8(trim_buf(&self.hits[i].title)).unwrap_or("")
    }
}

fn trim_buf(buf: &[u8]) -> &[u8] {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    &buf[..n]
}

fn copy_field(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let n = bytes.len().min(dst.len());
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
    let mut line = [0u8; 160];
    let Some(n) = com2.read_line(&mut line, TIMEOUT_PING) else {
        return MailPeek::empty(BridgeStatus::Offline);
    };
    let resp = core::str::from_utf8(&line[..n]).unwrap_or("");
    if !resp.starts_with("OK pong") {
        return MailPeek::empty(BridgeStatus::Offline);
    }

    if !caps.allows(crate::caps::Cap::EmailSearch) {
        // Bridge is up, but this guest was not granted inbox read.
        return MailPeek::empty(BridgeStatus::Online);
    }

    com2.write_str("CALL email.search q=in:inbox max=3\n");

    let mut peek = MailPeek::empty(BridgeStatus::Online);

    for _ in 0..16 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_LINE) else {
            break;
        };
        let resp = core::str::from_utf8(&line[..n]).unwrap_or("");
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
    let resp = core::str::from_utf8(&line[..n]).unwrap_or("");
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
    let mut line = [0u8; 200];

    match ping_bridge(&com2, &mut line) {
        BridgeStatus::Offline => return SearchPeek::empty(BridgeStatus::Offline, false),
        BridgeStatus::Online => {}
    }

    if !caps.allows(crate::caps::Cap::SearchQuery) {
        return SearchPeek::empty(BridgeStatus::Online, true);
    }

    // CALL search.query q=… k=3
    com2.write_str("CALL search.query q=");
    com2.write_str(q);
    com2.write_str(" k=3\n");

    let mut peek = SearchPeek::empty(BridgeStatus::Online, false);
    for _ in 0..16 {
        let Some(n) = com2.read_line(&mut line, TIMEOUT_LINE) else {
            break;
        };
        let resp = core::str::from_utf8(&line[..n]).unwrap_or("");
        if resp.starts_with("ERR ") || resp == "END" {
            break;
        }
        if resp.starts_with("OK search.query") {
            continue;
        }
        if resp.starts_with("ROW ") && peek.count < peek.hits.len() {
            let title = parse_row_field(resp, "title").unwrap_or("?");
            copy_field(&mut peek.hits[peek.count].title, title);
            peek.count += 1;
        }
    }
    peek
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_row() {
        let line = "ROW from=Alice Chen|subj=Q2 planning";
        assert_eq!(parse_row_field(line, "from"), Some("Alice Chen"));
        assert_eq!(parse_row_field(line, "subj"), Some("Q2 planning"));
    }

    #[test]
    fn parse_search_title() {
        let line = "ROW title=os identity|cat=docs|score=1.0|snip=hello|url=os://mock";
        assert_eq!(parse_row_field(line, "title"), Some("os identity"));
    }
}
