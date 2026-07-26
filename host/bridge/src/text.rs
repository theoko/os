//! Shared wire-text scrubbing and reply framing for guest-facing lines.

/// `OK …` header, zero or more body lines, then `END`.
pub(crate) fn framed_ok(header: String, rows: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = vec![header];
    out.extend(rows);
    out.push("END".into());
    out
}

/// Last non-empty-ish stderr line from a subprocess, capped for guest/ERR text.
pub(crate) fn stderr_brief(stderr: &[u8], fallback: &str, max: usize) -> String {
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
pub(crate) fn guest_slot(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| c.is_ascii())
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(max)
        .collect()
}
