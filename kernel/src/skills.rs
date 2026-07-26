//! Builtin skill catalog (always available offline) + optional bridge merge.

/// A skill name shown in the home UI.
pub(crate) struct SkillRef {
    pub name: &'static str,
    pub blurb: &'static str,
}

/// Defaults shipped with the OS (mirrors skills/defaults/).
pub(crate) const BUILTIN: &[SkillRef] = &[
    SkillRef {
        name: "agent-plan-act",
        blurb: "Plan, act, report",
    },
    SkillRef {
        name: "capability-safe-tools",
        blurb: "Least-privilege caps",
    },
    SkillRef {
        name: "email-triage",
        blurb: "Inbox via MCP email",
    },
    SkillRef {
        name: "inbox-brief",
        blurb: "Short mail brief",
    },
    SkillRef {
        name: "knowledge-search",
        blurb: "Corpus search",
    },
];

/// Names (+ short descs) from builtins or `CALL skills.list`.
pub struct SkillPeek {
    pub count: usize,
    /// True when the last fill came from the host bridge.
    pub from_bridge: bool,
    names: [[u8; 28]; 8],
    descs: [[u8; 40]; 8],
}

impl SkillPeek {
    pub fn empty() -> Self {
        Self {
            count: 0,
            from_bridge: false,
            names: [[0; 28]; 8],
            descs: [[0; 40]; 8],
        }
    }

    pub fn from_builtin() -> Self {
        let mut peek = Self::empty();
        for s in BUILTIN.iter().take(peek.names.len()) {
            peek.push(s.name, s.blurb);
        }
        peek
    }

    pub fn push(&mut self, name: &str, desc: &str) -> bool {
        if self.count >= self.names.len() {
            return false;
        }
        copy_field(&mut self.names[self.count], name);
        copy_field(&mut self.descs[self.count], desc);
        self.count += 1;
        true
    }

    pub fn name_at(&self, i: usize) -> &str {
        str_at(&self.names[i])
    }

    /// Desc when present; otherwise a source label for empty blurbs.
    pub fn subtitle_at(&self, i: usize) -> &str {
        let desc = str_at(&self.descs[i]);
        if !desc.is_empty() {
            return desc;
        }
        if self.from_bridge {
            "From host bridge"
        } else {
            "Shipped with the ISO"
        }
    }
}

/// Decode the longest valid UTF-8 prefix — a cut mid-character must degrade
/// to a shorter string, not vanish entirely.
pub(crate) fn utf8_prefix(bytes: &[u8]) -> &str {
    match core::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    }
}

/// Null-terminated fixed field as a string (UTF-8 prefix).
pub(crate) fn str_at(buf: &[u8]) -> &str {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    utf8_prefix(&buf[..n])
}

/// Copy `src` into a fixed field, never splitting a UTF-8 char.
pub(crate) fn copy_field(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let mut n = bytes.len().min(dst.len());
    // Never cut mid-character: a torn tail would make the whole field
    // undecodable when read back.
    while n > 0 && !src.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_present() {
        let p = SkillPeek::from_builtin();
        assert!(p.count >= 5);
        assert_eq!(p.name_at(0), "agent-plan-act");
        assert!(!p.from_bridge);
    }

    #[test]
    fn push_caps_at_slot_limit() {
        let mut p = SkillPeek::empty();
        for i in 0..8 {
            assert!(p.push("n", "d"), "slot {i}");
        }
        assert!(!p.push("overflow", "no"));
        assert_eq!(p.count, 8);
    }
}
