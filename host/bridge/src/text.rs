//! Shared wire-text scrubbing and reply framing for guest-facing lines.

/// `OK …` header, zero or more body lines, then `END`.
pub fn framed_ok(header: String, rows: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = vec![header];
    out.extend(rows);
    out.push("END".into());
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
        out = out.trim().to_string();
    }
    out
}
