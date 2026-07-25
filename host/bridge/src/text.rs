//! Shared wire-text scrubbing for guest-facing ROW fields.

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
