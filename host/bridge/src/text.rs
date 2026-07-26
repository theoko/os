//! Shared wire-text scrubbing and reply framing for guest-facing lines.

/// `OK …` header, zero or more body lines, then `END`.
pub fn framed_ok(header: String, rows: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = vec![header];
    out.extend(rows);
    out.push("END".into());
    out
}

/// Last non-empty-ish stderr line from a subprocess, capped for guest/ERR text.
pub fn stderr_brief(stderr: &[u8], fallback: &str, max: usize) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .last()
        .unwrap_or(fallback)
        .chars()
        .take(max)
        .collect()
}

/// Trim leading/trailing whitespace without a second allocation.
pub(crate) fn trim_in_place(s: &mut String) {
    let end = s.trim_end().len();
    s.truncate(end);
    let lead = s.len() - s.trim_start().len();
    if lead > 0 {
        s.drain(..lead);
    }
}

/// ASCII guest slot (font atlas 0x20..=0x7E); no trim — ROW fields keep spaces.
pub fn guest_slot(s: &str, max: usize) -> String {
    sanitize(s, max, true, false)
}

/// One pass over a `ROW k=v|…` line for `N` keys (same shape as guest `mcp::parse_row`).
pub fn parse_row<'a, const N: usize>(line: &'a str, keys: [&str; N]) -> [Option<&'a str>; N] {
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

/// Map line breaks / pipes / controls to spaces, optionally drop non-ASCII,
/// then take at most `max` chars (and optionally trim).
pub fn sanitize(s: &str, max: usize, ascii_only: bool, trim: bool) -> String {
    let mut out: String = s
        .chars()
        .filter(|c| !ascii_only || c.is_ascii())
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(max)
        .collect();
    if trim {
        trim_in_place(&mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_row_extracts_keys() {
        assert_eq!(
            parse_row("ROW from=ada@x|subj=Hello", ["from", "subj"]),
            [Some("ada@x"), Some("Hello")]
        );
        assert_eq!(
            parse_row("ROW from=ada@x", ["from", "subj"]),
            [Some("ada@x"), None]
        );
        assert_eq!(parse_row("OK email.search", ["from", "subj"]), [None, None]);
    }
}
