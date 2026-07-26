//! Builtin skill catalog (always available offline) + optional bridge list.

/// Cap on bridge-listed skills (buffer, paint, home count, bridge `.take`).
pub(crate) const MAX_LISTED: usize = 6;

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

/// Guest `Slot` / bridge `sanitize_slot` budgets.
pub(crate) const NAME_CHARS: usize = 28;
pub(crate) const DESC_CHARS: usize = 40;

/// One skill row from `CALL skills.list`.
struct Slot {
    name: [u8; NAME_CHARS],
    desc: [u8; DESC_CHARS],
}

const EMPTY_SLOT: Slot = Slot {
    name: [0; NAME_CHARS],
    desc: [0; DESC_CHARS],
};

enum Kind {
    /// [`BUILTIN`] — offline / ERR fallback (not a live empty list).
    Builtin,
    /// Rows from `CALL skills.list`.
    Listed { count: usize, slots: [Slot; MAX_LISTED] },
}

/// Names (+ short descs) from ISO builtins or a live bridge list.
pub struct SkillPeek {
    kind: Kind,
}

impl SkillPeek {
    /// Empty listed peek ready for [`Self::push`] (bridge fill path).
    pub(crate) fn empty() -> Self {
        Self {
            kind: Kind::Listed {
                count: 0,
                slots: [EMPTY_SLOT; MAX_LISTED],
            },
        }
    }

    pub fn from_builtin() -> Self {
        Self {
            kind: Kind::Builtin,
        }
    }

    /// True when the last fill came from the host bridge.
    pub fn from_bridge(&self) -> bool {
        matches!(self.kind, Kind::Listed { .. })
    }

    pub(crate) fn count(&self) -> usize {
        match &self.kind {
            Kind::Builtin => BUILTIN.len(),
            Kind::Listed { count, .. } => *count,
        }
    }

    pub(crate) fn push(&mut self, name: &str, desc: &str) -> bool {
        let Kind::Listed { count, slots } = &mut self.kind else {
            return false;
        };
        if *count >= slots.len() {
            return false;
        }
        let slot = &mut slots[*count];
        copy_field(&mut slot.name, name);
        copy_field(&mut slot.desc, desc);
        *count += 1;
        true
    }

    pub(crate) fn name_at(&self, i: usize) -> &str {
        match &self.kind {
            Kind::Builtin => BUILTIN.get(i).map(|s| s.name).unwrap_or(""),
            Kind::Listed { count, slots } => {
                if i < *count {
                    str_at(&slots[i].name)
                } else {
                    ""
                }
            }
        }
    }

    /// Desc when present; otherwise a source label for empty blurbs.
    pub(crate) fn subtitle_at(&self, i: usize) -> &str {
        match &self.kind {
            Kind::Builtin => BUILTIN.get(i).map(|s| s.blurb).unwrap_or("Shipped with the ISO"),
            Kind::Listed { count, slots } => {
                if i >= *count {
                    return "From host bridge";
                }
                let desc = str_at(&slots[i].desc);
                if desc.is_empty() {
                    "From host bridge"
                } else {
                    desc
                }
            }
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
        assert!(p.count() >= 5);
        assert_eq!(p.name_at(0), "agent-plan-act");
        assert!(!p.from_bridge());
    }

    #[test]
    fn push_caps_at_slot_limit() {
        let mut p = SkillPeek::empty();
        for i in 0..MAX_LISTED {
            assert!(p.push("n", "d"), "slot {i}");
        }
        assert!(!p.push("overflow", "no"));
        assert_eq!(p.count(), MAX_LISTED);
        assert!(p.from_bridge());
    }

    #[test]
    fn empty_listed_is_still_from_bridge() {
        let p = SkillPeek::empty();
        assert_eq!(p.count(), 0);
        assert!(p.from_bridge(), "framed empty list is not ISO builtins");
    }
}
